use anyhow::Error;
use docker_api::{
    api::{ImageBuildChunk, ImageId},
    Docker,
};
use futures::StreamExt;
use log::error;
use tokio::sync::mpsc;

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub enum ImageImportEvent {
    PollData,
    Kill,
}

#[derive(Debug)]
pub struct ImageImportWorker {
    pub image_archive: std::path::PathBuf,
    pub rx_events: mpsc::Receiver<ImageImportEvent>,
    pub tx_results: mpsc::Sender<anyhow::Result<ImageId>>,
    pub tx_chunks: mpsc::Sender<Vec<ImageBuildChunk>>,
}

impl ImageImportWorker {
    #[allow(clippy::type_complexity)] // TODO: temporarily
    pub fn new(
        image_archive: impl AsRef<std::path::Path>,
    ) -> (
        Self,
        mpsc::Sender<ImageImportEvent>,
        mpsc::Receiver<Vec<ImageBuildChunk>>,
        mpsc::Receiver<anyhow::Result<ImageId>>,
    ) {
        let (tx_results, rx_results) = mpsc::channel::<anyhow::Result<ImageId>>(128);
        let (tx_chunks, rx_chunks) = mpsc::channel::<Vec<ImageBuildChunk>>(128);
        let (tx_events, rx_events) = mpsc::channel::<ImageImportEvent>(128);

        (
            Self {
                image_archive: image_archive.as_ref().to_path_buf(),
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
        log::trace!("starting image `{}` import", self.image_archive.display());
        let archive = match std::fs::File::open(&self.image_archive) {
            Ok(f) => f,
            Err(e) => {
                if let Err(e) = self
                    .tx_results
                    .send(Err(Error::msg(format!(
                        "failed to open image archive at {}: {}",
                        self.image_archive.display(),
                        e
                    ))))
                    .await
                {
                    error!("failed to send image import result: {}", e);
                };
                return;
            }
        };
        let mut import_stream = docker.images().import(archive);
        let mut chunks = Box::new(vec![]);

        macro_rules! send_chunks {
            () => {
                if let Err(e) = self.tx_chunks.try_send(std::mem::take(&mut chunks)) {
                    error!("failed to send image import chunks: {}", e);
                }
            };
        }
        loop {
            tokio::select! {
                chunk = import_stream.next() => {
                        match chunk {
                            Some(Ok(chunk)) => {
                                log::trace!("{:?}", chunk);
                                let c = chunk.clone();

                                log::trace!("adding chunk");
                                chunks.push(chunk);

                                match c {
                                    ImageBuildChunk::Update { stream } => {
                                        const LOADED_IMAGE: &str = "Loaded image ID: ";
                                        if stream.starts_with(LOADED_IMAGE) {
                                            let _ = self.tx_results
                                                .send(Ok(stream.trim_start_matches(LOADED_IMAGE).to_string()))
                                                .await;

                                            send_chunks!();
                                            break;
                                        }
                                    }
                                    ImageBuildChunk::Error { error, error_detail: _ } => {
                                            let _ = self.tx_results
                                                .send(Err(Error::msg(error)))
                                                .await;

                                            send_chunks!();
                                            break;
                                    }
                                    _ => {}
                                };
                            }
                            Some(Err(e)) => {
                                match e {
                                    docker_api::Error::Fault {
                                        code: http::status::StatusCode::NOT_FOUND, message: _
                                    } => break,
                                    e => error!("failed to read image import chunk: {}", e),
                                }
                            }
                            None => {
                        log::trace!(
                                "image `{}` import finished successfuly",
                                self.image_archive.display()
                        );
                        let Self { image_archive, tx_results, .. }  = self;
                        let _ = tx_results
                            .send(Ok(image_archive.to_string_lossy().to_string()))
                            .await;
                        return;
                            }
                    }
                }
                event = self.rx_events.recv() => {
                    match event {
                        Some(ImageImportEvent::PollData) => send_chunks!(),
                        Some(ImageImportEvent::Kill) => break,
                        None => continue,
                    }
                }
            }
        }
        send_chunks!();
    }
}
