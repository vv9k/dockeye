use anyhow::Error;
use docker_api::Docker;
use futures::StreamExt;
use log::error;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use tokio::sync::mpsc;

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub enum ImageExportEvent {
    Kill,
}

#[derive(Debug)]
pub struct ImageExportWorker {
    pub image_id: String,
    pub output_path: PathBuf,
    pub rx_events: mpsc::Receiver<ImageExportEvent>,
    pub tx_results: mpsc::Sender<anyhow::Result<(String, PathBuf)>>,
}

impl ImageExportWorker {
    pub fn new(
        image_id: String,
        output_path: PathBuf,
    ) -> (
        Self,
        mpsc::Sender<ImageExportEvent>,
        mpsc::Receiver<anyhow::Result<(String, PathBuf)>>,
    ) {
        let (tx_results, rx_results) = mpsc::channel::<anyhow::Result<(String, PathBuf)>>(128);
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
        log::trace!("starting image `{}` export", self.image_id);
        let image = docker.images().get(&self.image_id);
        let mut export_stream = image.export();
        let mut export_file = match OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.output_path)
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
                                log::trace!("saving image export chunk");
                                if let Err(e) = export_file.write(&chunk) {
                                    error!("{}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                match e {
                                    docker_api::Error::Fault {
                                        code: http::status::StatusCode::NOT_FOUND, message: _
                                    } => break,
                                    e => error!("failed to read image export chunk: {}", e),
                                }
                            }
                        }
                    } else {
                        log::trace!(
                                "image `{}` export finished successfuly",
                                self.image_id
                        );
                        let Self { image_id, tx_results, output_path, .. }  = self;
                        let _ = tx_results
                            .send(Ok((image_id, output_path)))
                            .await;
                        return;
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
