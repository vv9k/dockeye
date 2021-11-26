use anyhow::Error;
use docker_api::Docker;
use futures::StreamExt;
use log::error;
use tokio::sync::mpsc;
use tokio::{fs::OpenOptions, io::AsyncWriteExt};

#[derive(Debug, PartialEq)]
pub enum ImageExportEvent {
    Kill,
}

#[derive(Debug)]
pub struct ImageExportWorker {
    pub image_id: String,
    pub output_path: std::path::PathBuf,
    pub rx_events: mpsc::Receiver<ImageExportEvent>,
    pub tx_results: mpsc::Sender<anyhow::Result<()>>,
}

impl ImageExportWorker {
    pub fn new(
        image_id: String,
        output_path: std::path::PathBuf,
    ) -> (
        Self,
        mpsc::Sender<ImageExportEvent>,
        mpsc::Receiver<anyhow::Result<()>>,
    ) {
        let (tx_results, rx_results) = mpsc::channel::<anyhow::Result<()>>(128);
        let (tx_events, rx_events) = mpsc::channel::<ImageExportEvent>(128);

        (
            Self {
                image_id,
                output_path,
                rx_events,
                tx_results,
            },
            tx_events,
            rx_results,
        )
    }
    pub async fn work(mut self, docker: Docker) {
        let image = docker.images().get(&self.image_id);
        let mut export_stream = image.export();
        let mut export_file = match OpenOptions::new()
            .write(true)
            .create(true)
            .open(&self.output_path)
            .await
        {
            Ok(f) => f,
            Err(e) => {
                let _ = self
                    .tx_results
                    .send(Err(Error::msg(format!(
                        "opening file to export image failed - {}",
                        e
                    ))))
                    .await;
                return;
            }
        };
        loop {
            tokio::select! {
                bytes = export_stream.next() => {
                    if let Some(data) = bytes {
                        match data {
                            Ok(chunk) => {
                                log::trace!("saving export image chunk");
                                if let Err(e) = export_file.write(&chunk).await {
                                    error!("{}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                match e {
                                    docker_api::Error::Fault {
                                        code: http::status::StatusCode::NOT_FOUND, message: _
                                    } => break,
                                    e => error!("failed to read container logs: {}", e),
                                }
                            }
                        }
                    } else {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
                event = self.rx_events.recv() => {
                    match event {
                        Some(ImageExportEvent::Kill) => break,
                        None => continue,

                    }
                }
            }
        }
    }
}
