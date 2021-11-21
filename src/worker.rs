use crate::event::{EventRequest, EventResponse, ImageInspectInfo};
use crate::stats::{worker::StatsWorker, StatsWrapper};

use anyhow::Result;
use docker_api::Docker;
use log::{debug, error, trace};
use std::time::Duration;
use tokio::sync::mpsc;

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
                        error!("[listener] failed to send request to worker task: {}", e);
                    }
                }
            }
        });
        let worker = tokio::spawn(async move {
            let (tx_stats, mut rx_stats) = mpsc::channel::<Vec<(Duration, StatsWrapper)>>(8);
            let (tx_want_data, rx_want_data) = mpsc::channel::<()>(64);
            let (tx_id, rx_id) = mpsc::channel::<String>(16);
            let stats_worker = StatsWorker {
                rx_id,
                tx_stats,
                rx_want_data,
                docker: docker.clone(),
            };
            let _ = tokio::spawn(stats_worker.work());
            loop {
                if let Some(req) = inner_rx_req.recv().await {
                    debug!("[worker] got request: {:?}", req);
                    let rsp = match req {
                        EventRequest::ListContainers(opts) => {
                            let opts = opts.unwrap_or_default();
                            match docker.containers().list(&opts).await {
                                Ok(containers) => EventResponse::ListContainers(containers),
                                Err(e) => {
                                    error!("[worker] failed to list containers: {}", e);
                                    continue;
                                }
                            }
                        }
                        EventRequest::ListImages(opts) => {
                            let opts = opts.unwrap_or_default();
                            match docker.images().list(&opts).await {
                                Ok(images) => EventResponse::ListImages(images),
                                Err(e) => {
                                    error!("[worker] failed to list images: {}", e);
                                    continue;
                                }
                            }
                        }
                        EventRequest::InspectContainer { id } => {
                            match docker.containers().get(id).inspect().await {
                                Ok(container) => {
                                    EventResponse::InspectContainer(Box::new(container))
                                }
                                Err(e) => {
                                    error!("[worker] failed to inspect a container: {}", e);
                                    continue;
                                }
                            }
                        }
                        EventRequest::InspectImage { id } => {
                            let image = docker.images().get(id);
                            let details = match image.inspect().await {
                                Ok(details) => details,
                                Err(e) => {
                                    error!("[worker] failed to inspect an image: {}", e);
                                    continue;
                                }
                            };
                            let distribution_info = match image.distribution_inspect().await {
                                Ok(info) => Some(info),
                                Err(e) => {
                                    trace!("[worker] failed to inspect image distribution: {}", e);
                                    None
                                }
                            };
                            let history = match image.history().await {
                                Ok(history) => history,
                                Err(e) => {
                                    error!("[worker] failed to check image history: {}", e);
                                    continue;
                                }
                            };
                            EventResponse::InspectImage(Box::new(ImageInspectInfo {
                                details,
                                distribution_info,
                                history,
                            }))
                        }
                        EventRequest::DeleteContainer { id } => {
                            match docker.containers().get(&id).delete().await {
                                Ok(msg) => {
                                    let msg = if msg.is_empty() {
                                        format!("[worker] successfully deleted container {}", id)
                                    } else {
                                        msg
                                    };
                                    EventResponse::DeleteContainer(msg)
                                }
                                Err(e) => {
                                    error!("[worker] failed to delete a container: {}", e);
                                    continue;
                                }
                            }
                        }
                        EventRequest::DeleteImage { id } => {
                            match docker.images().get(&id).delete().await {
                                Ok(status) => EventResponse::DeleteImage(status),
                                Err(e) => {
                                    error!("[worker] failed to delete a image: {}", e);
                                    continue;
                                }
                            }
                        }
                        EventRequest::ContainerStatsStart { id } => {
                            if let Err(e) = tx_id.send(id).await {
                                error!("[worker] failed to start stats collection: {}", e);
                            }
                            continue;
                        }
                        EventRequest::ContainerStats => {
                            if let Err(e) = tx_want_data.send(()).await {
                                error!("[worker] failed to collect stats data: {}", e);
                                continue;
                            }
                            trace!("[worker] notified stats worker to poll data, reading stats");
                            if let Some(stats) = rx_stats.recv().await {
                                trace!("[worker] got data {:?}", stats);
                                EventResponse::ContainerStats(stats)
                            } else {
                                log::warn!("[worker] no stats available");
                                continue;
                            }
                        }
                        EventRequest::PauseContainer { id } => {
                            EventResponse::PauseContainer(docker.containers().get(id).pause().await)
                        }
                        EventRequest::UnpauseContainer { id } => EventResponse::UnpauseContainer(
                            docker.containers().get(id).unpause().await,
                        ),
                        EventRequest::StopContainer { id } => EventResponse::StopContainer(
                            docker
                                .containers()
                                .get(id)
                                .stop(Some(Duration::from_millis(0)))
                                .await,
                        ),
                        EventRequest::StartContainer { id } => {
                            EventResponse::StartContainer(docker.containers().get(id).start().await)
                        }
                    };
                    debug!("[worker] sending response");
                    //trace!("[worker] {:?}", rsp);

                    let _ = tx_rsp.send(rsp).await;
                }
            }
        });
        let _ = tokio::join!(listener, worker);
        Ok(())
    }
}
