mod app;
pub use app::App;

use anyhow::Result;
use docker_api::api::{ContainerInfo, ContainerListOpts, ImageInfo, ImageListOpts};
use docker_api::Docker;
use log::{debug, error, trace};
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum EventRequest {
    ListContainers(Option<ContainerListOpts>),
    ListImages(Option<ImageListOpts>),
}

#[derive(Debug)]
pub enum EventResponse {
    ListContainers(Vec<ContainerInfo>),
    ListImages(Vec<ImageInfo>),
}

pub struct DockerWorker {
    docker: Docker,
    rx_req: mpsc::Receiver<EventRequest>,
    tx_rsp: mpsc::Sender<EventResponse>,
}

impl DockerWorker {
    pub fn new(
        uri: impl AsRef<str>,
        rx_req: mpsc::Receiver<EventRequest>,
        tx_rsp: mpsc::Sender<EventResponse>,
    ) -> Result<Self> {
        Ok(DockerWorker {
            docker: Docker::new(uri)?,
            rx_req,
            tx_rsp,
        })
    }
    pub async fn work(self) -> Result<()> {
        let Self {
            docker,
            mut rx_req,
            tx_rsp,
        } = self;
        let (inner_tx_req, mut inner_rx_req) = mpsc::channel::<EventRequest>(64);

        let listener = tokio::spawn(async move {
            loop {
                if let Some(req) = rx_req.recv().await {
                    debug!("[listener] got request: {:?}", req);
                    if let Err(e) = inner_tx_req.send(req).await {
                        error!("failed to send request to worker task: {}", e);
                    }
                }
            }
        });
        let worker = tokio::spawn(async move {
            loop {
                if let Some(req) = inner_rx_req.recv().await {
                    debug!("[worker] got request: {:?}", req);
                    let rsp = match req {
                        EventRequest::ListContainers(opts) => {
                            let opts = opts.unwrap_or_default();
                            match docker.containers().list(&opts).await {
                                Ok(containers) => EventResponse::ListContainers(containers),
                                Err(e) => {
                                    error!("failed to list containers: {}", e);
                                    continue;
                                }
                            }
                        }
                        EventRequest::ListImages(opts) => {
                            let opts = opts.unwrap_or_default();
                            match docker.images().list(&opts).await {
                                Ok(images) => EventResponse::ListImages(images),
                                Err(e) => {
                                    error!("failed to list images: {}", e);
                                    continue;
                                }
                            }
                        }
                    };
                    debug!("[worker] sending response");
                    trace!("{:?}", rsp);

                    let _ = tx_rsp.send(rsp).await;
                }
            }
        });
        let _ = tokio::join!(listener, worker);
        Ok(())
    }
}
