mod logs;
mod stats;

use crate::event::{EventRequest, EventResponse, ImageInspectInfo};
pub use logs::{LogWorkerEvent, Logs, LogsWorker};
pub use stats::{RunningContainerStats, StatsWorker, StatsWorkerEvent};

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
        uri: String,
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
            let mut current_id = None;
            let (_, mut tx_stats_event, mut rx_stats) = StatsWorker::new("");
            let (_, mut tx_logs_event, mut rx_logs) = LogsWorker::new("");

            loop {
                if let Some(req) = inner_rx_req.recv().await {
                    let event_str = format!("{:?}", req);
                    debug!("got request: {}", event_str);
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
                        EventRequest::ContainerTraceStart { id } => {
                            if Some(&id) == current_id.as_ref() {
                                continue;
                            }

                            if current_id.is_some() {
                                if let Err(e) = tx_logs_event.send(LogWorkerEvent::Kill).await {
                                    error!("failed to send kill event to log worker: {}", e);
                                }
                                if let Err(e) = tx_stats_event.send(StatsWorkerEvent::Kill).await {
                                    error!("failed to send kill event to stats worker: {}", e);
                                }
                            }

                            current_id = Some(id.clone());

                            let s = StatsWorker::new(&id);
                            tx_stats_event = s.1;
                            rx_stats = s.2;
                            let _ = tokio::spawn(s.0.work(docker.clone()));

                            let w = LogsWorker::new(&id);
                            tx_logs_event = w.1;
                            rx_logs = w.2;
                            let _ = tokio::spawn(w.0.work(docker.clone()));

                            match docker.containers().get(&id).inspect().await {
                                Ok(container) => {
                                    EventResponse::ContainerDetails(Box::new(container))
                                }
                                Err(e) => {
                                    error!("failed to inspect a container: {}", e);
                                    continue;
                                }
                            }
                        }
                        EventRequest::ContainerDetails => {
                            if let Some(id) = &current_id {
                                match docker.containers().get(id).inspect().await {
                                    Ok(container) => {
                                        EventResponse::ContainerDetails(Box::new(container))
                                    }
                                    Err(e) => match e {
                                        docker_api::Error::Fault {
                                            code: http::status::StatusCode::NOT_FOUND,
                                            message: _,
                                        } => EventResponse::InspectContainerNotFound,
                                        e => {
                                            error!("failed to inspect container: {}", e);
                                            continue;
                                        }
                                    },
                                }
                            } else {
                                error!("failed to inspect a container: no current id set");
                                continue;
                            }
                        }
                        EventRequest::InspectImage { id } => {
                            let image = docker.images().get(id);
                            let details = match image.inspect().await {
                                Ok(details) => details,
                                Err(e) => {
                                    error!("failed to inspect an image: {}", e);
                                    continue;
                                }
                            };
                            let distribution_info = match image.distribution_inspect().await {
                                Ok(info) => Some(info),
                                Err(e) => {
                                    trace!("failed to inspect image distribution: {}", e);
                                    None
                                }
                            };
                            let history = match image.history().await {
                                Ok(history) => history,
                                Err(e) => {
                                    error!("failed to check image history: {}", e);
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
                                        format!("successfully deleted container {}", id)
                                    } else {
                                        msg
                                    };
                                    EventResponse::DeleteContainer(msg)
                                }
                                Err(e) => {
                                    error!("failed to delete a container: {}", e);
                                    continue;
                                }
                            }
                        }
                        EventRequest::DeleteImage { id } => {
                            match docker.images().get(&id).delete().await {
                                Ok(status) => EventResponse::DeleteImage(status),
                                Err(e) => {
                                    error!("failed to delete a image: {}", e);
                                    continue;
                                }
                            }
                        }
                        EventRequest::ContainerStats => {
                            if let Err(e) = tx_stats_event.send(StatsWorkerEvent::PollData).await {
                                error!("failed to collect stats data: {}", e);
                                continue;
                            }
                            trace!("notified stats worker to poll data, reading stats");
                            if let Some(stats) = rx_stats.recv().await {
                                trace!("got data {:?}", stats);
                                EventResponse::ContainerStats(stats)
                            } else {
                                log::warn!("no stats available");
                                continue;
                            }
                        }
                        EventRequest::ContainerLogs => {
                            if let Err(e) = tx_logs_event.send(LogWorkerEvent::PollData).await {
                                error!("failed to collect logs: {}", e);
                                continue;
                            }
                            trace!("notified logs worker to poll data, reading logs");
                            if let Some(logs) = rx_logs.recv().await {
                                trace!("got data {:?}", logs);
                                EventResponse::ContainerLogs(logs)
                            } else {
                                log::warn!("no logs available");
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
                    debug!("sending response to event: {}", event_str);
                    //trace!("{:?}", rsp);

                    let _ = tx_rsp.send(rsp).await;
                }
            }
        });
        let _ = tokio::join!(listener, worker);
        Ok(())
    }

    pub fn spawn(
        runtime: tokio::runtime::Runtime,
        docker_addr: String,
        rx_req: mpsc::Receiver<EventRequest>,
        tx_rsp: mpsc::Sender<EventResponse>,
    ) {
        std::thread::spawn(move || -> Result<()> {
            let worker = DockerWorker::new(docker_addr, rx_req, tx_rsp)?;

            runtime.block_on(worker.work())
        });
    }
}
