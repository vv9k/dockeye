use anyhow::Error;
use docker_api::{
    api::{ImageBuildChunk, PullOpts, RegistryAuth},
    Docker,
};
use futures::StreamExt;
use log::error;
use tokio::sync::mpsc;

#[derive(Debug, PartialEq)]
pub enum ImagePullEvent {
    PollData,
    #[allow(dead_code)]
    Kill,
}

#[derive(Debug)]
pub struct ImagePullWorker {
    pub image_id: String,
    pub auth: Option<RegistryAuth>,
    pub rx_events: mpsc::Receiver<ImagePullEvent>,
    pub tx_results: mpsc::Sender<anyhow::Result<String>>,
    pub tx_chunks: mpsc::Sender<Vec<ImageBuildChunk>>,
}

impl ImagePullWorker {
    #[allow(clippy::type_complexity)] // TODO: temporarily
    pub fn new(
        image_id: String,
        auth: Option<RegistryAuth>,
    ) -> (
        Self,
        mpsc::Sender<ImagePullEvent>,
        mpsc::Receiver<Vec<ImageBuildChunk>>,
        mpsc::Receiver<anyhow::Result<String>>,
    ) {
        let (tx_results, rx_results) = mpsc::channel::<anyhow::Result<String>>(128);
        let (tx_chunks, rx_chunks) = mpsc::channel::<Vec<ImageBuildChunk>>(128);
        let (tx_events, rx_events) = mpsc::channel::<ImagePullEvent>(128);

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
                    if let Some(chunk) = chunk {
                        match chunk {
                            Ok(chunk) => {
                                log::trace!("{:?}", chunk);
                                let c = chunk.clone();

                                log::trace!("adding chunk");
                                chunks.push(chunk);

                                match c {
                                    ImageBuildChunk::PullStatus { status, ..} => {
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
                                        } else {
                                        }
                                    }
                                    _ => {}
                                };
                            }
                            Err(e) => {
                                match e {
                                    docker_api::Error::Fault {
                                        code: http::status::StatusCode::NOT_FOUND, message: _
                                    } => break,
                                    e => error!("failed to read image pullchunk: {}", e),
                                }
                            }
                    }
                } else {
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
                event = self.rx_events.recv() => {
                    match event {
                        Some(ImagePullEvent::PollData) =>
                        if let Err(e) = self.tx_chunks.send(std::mem::take(&mut chunks)).await {
                            error!("failed to send image pull chunks: {}", e);
                        },
                        Some(ImagePullEvent::Kill) => break,
                        None => continue,
                    }
                }
            }
        }
    }
}
