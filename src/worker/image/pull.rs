use crate::worker::WorkerEvent;

use anyhow::Error;
use docker_api::{
    api::{ImageBuildChunk, ImageId, PullOpts, RegistryAuth},
    Docker,
};
use futures::StreamExt;
use log::error;
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct ImagePullWorker {
    pub image_id: ImageId,
    pub auth: Option<RegistryAuth>,
    pub rx_events: mpsc::Receiver<WorkerEvent>,
    pub tx_results: mpsc::Sender<anyhow::Result<ImageId>>,
    pub tx_chunks: mpsc::Sender<Vec<ImageBuildChunk>>,
}

impl ImagePullWorker {
    #[allow(clippy::type_complexity)] // TODO: temporarily
    pub fn new(
        image_id: ImageId,
        auth: Option<RegistryAuth>,
    ) -> (
        Self,
        mpsc::Sender<WorkerEvent>,
        mpsc::Receiver<Vec<ImageBuildChunk>>,
        mpsc::Receiver<anyhow::Result<ImageId>>,
    ) {
        let (tx_results, rx_results) = mpsc::channel::<anyhow::Result<ImageId>>(128);
        let (tx_chunks, rx_chunks) = mpsc::channel::<Vec<ImageBuildChunk>>(128);
        let (tx_events, rx_events) = mpsc::channel::<WorkerEvent>(128);

        (
            Self {
                image_id,
                auth,
                rx_events,
                tx_chunks,
                tx_results,
            },
            tx_events,
            rx_chunks,
            rx_results,
        )
    }
    pub async fn work(mut self, docker: Docker) {
        log::trace!("starting image `{}` pull", self.image_id);
        let opts = if let Some(auth) = self.auth {
            PullOpts::builder().image(&self.image_id).auth(auth).build()
        } else {
            PullOpts::builder().image(&self.image_id).build()
        };
        let mut pull_stream = docker.images().pull(&opts);
        let mut chunks = Box::new(vec![]);

        macro_rules! send_chunks {
            () => {
                if let Err(e) = self.tx_chunks.try_send(std::mem::take(&mut chunks)) {
                    error!("failed to send image pull chunks: {}", e);
                }
            };
        }
        loop {
            tokio::select! {
                chunk = pull_stream.next() => {
                    match chunk {
                        Some(Ok(chunk)) => {
                            log::trace!("{:?}", chunk);
                            let c = chunk.clone();

                            log::trace!("adding chunk");
                            chunks.push(chunk);

                            if let ImageBuildChunk::PullStatus { status, ..} = c {
                                if status.contains("Digest:") {
                                    let _ = self.tx_results
                                        .send(Ok(status.trim_start_matches("Digest: ").to_string()))
                                        .await;

                                    send_chunks!();
                                    break;
                                } else if status.contains("error") {
                                    let _ = self.tx_results
                                        .send(Err(Error::msg(status.clone())))
                                        .await;

                                    send_chunks!();
                                    break;
                                }
                            }
                        }
                        Some(Err(e)) => {
                            match e {
                                docker_api::Error::Fault {
                                    code: http::status::StatusCode::NOT_FOUND, message: _
                                } => break,
                                e => error!("failed to read image pullchunk: {}", e),
                            }
                        }
                        None => {
                            log::trace!(
                                    "image `{}` pull finished successfuly",
                                    self.image_id
                            );
                            let Self { image_id, tx_results, .. }  = self;
                            let _ = tx_results
                                .send(Ok(image_id))
                                .await;
                            return;
                        }
                    }
                }
                event = self.rx_events.recv() => {
                    match event {
                        Some(WorkerEvent::PollData) =>
                        if let Err(e) = self.tx_chunks.send(std::mem::take(&mut chunks)).await {
                            error!("failed to send image pull chunks: {}", e);
                        },
                        Some(WorkerEvent::Kill) => break,
                        None => continue,
                    }
                }
            }
        }
    }
}
