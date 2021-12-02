mod image_export;
mod image_pull;
mod logs;
mod stats;

use crate::event::{EventRequest, EventResponse, ImageInspectInfo, SystemInspectInfo};
pub use image_export::{ImageExportEvent, ImageExportWorker};
pub use image_pull::{ImagePullEvent, ImagePullWorker};
pub use logs::{LogWorkerEvent, Logs, LogsWorker};
pub use stats::{RunningContainerStats, StatsWorker, StatsWorkerEvent};

use anyhow::{Context, Result};
use docker_api::Docker;
use log::{debug, error, trace};
use std::time::Duration;
use tokio::sync::mpsc;

pub struct DockerWorker {
    docker: Docker,
    uri: String,
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
            docker: Docker::new(&uri)?,
            uri,
            rx_req,
            tx_rsp,
        })
    }
    pub async fn work(self) -> Result<()> {
        let Self {
            mut docker,
            uri: mut current_uri,
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
            if let Err(e) = docker.adjust_api_version().await {
                error!("failed to adjust docker API version: {}", e);
            }
            let mut current_id = None;
            let (_, mut tx_stats_event, mut rx_stats) = StatsWorker::new("");
            let (_, mut tx_logs_event, mut rx_logs) = LogsWorker::new("");
            let (_, mut _tx_image_export_event, mut rx_image_export_results) =
                ImageExportWorker::new("".to_string(), std::path::PathBuf::new());
            let (_, mut tx_pull_event, mut rx_chunks, mut rx_image_pull_results) =
                ImagePullWorker::new("".to_string(), None);
            let mut image_export_in_progress = false;
            let mut image_pull_in_progress = false;

            loop {
                if image_export_in_progress {
                    if let Ok(res) = rx_image_export_results.try_recv() {
                        let rsp = EventResponse::SaveImage(res);
                        let _ = tx_rsp.send(rsp).await;
                        image_export_in_progress = false;
                    }
                }
                if image_pull_in_progress {
                    if let Ok(res) = rx_image_pull_results.try_recv() {
                        let rsp = EventResponse::PullImage(res);
                        let _ = tx_rsp.send(rsp).await;
                        if let Some(chunks) = rx_chunks.recv().await {
                            let rsp = EventResponse::PullImageChunks(chunks);
                            let _ = tx_rsp.send(rsp).await;
                        }
                        image_pull_in_progress = false;
                    }
                }
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
                        EventRequest::DeleteContainer { id } => EventResponse::DeleteContainer(
                            docker
                                .containers()
                                .get(&id)
                                .delete()
                                .await
                                .map(|_| id)
                                .context("deleting container"),
                        ),
                        EventRequest::DeleteImage { id } => EventResponse::DeleteImage(
                            docker
                                .images()
                                .get(&id)
                                .delete()
                                .await
                                .context("deleting image"),
                        ),
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
                        EventRequest::PauseContainer { id } => EventResponse::PauseContainer(
                            docker
                                .containers()
                                .get(id)
                                .pause()
                                .await
                                .context("pausing container"),
                        ),
                        EventRequest::UnpauseContainer { id } => EventResponse::UnpauseContainer(
                            docker
                                .containers()
                                .get(id)
                                .unpause()
                                .await
                                .context("unpausing container"),
                        ),
                        EventRequest::StopContainer { id } => EventResponse::StopContainer(
                            docker
                                .containers()
                                .get(id)
                                .stop(Some(Duration::from_millis(0)))
                                .await
                                .context("stopping container"),
                        ),
                        EventRequest::StartContainer { id } => EventResponse::StartContainer(
                            docker
                                .containers()
                                .get(id)
                                .start()
                                .await
                                .context("starting container"),
                        ),
                        EventRequest::SaveImage { id, output_path } => {
                            let d = docker.clone();
                            let i = ImageExportWorker::new(id, output_path);
                            _tx_image_export_event = i.1;
                            rx_image_export_results = i.2;
                            tokio::task::spawn(async move {
                                i.0.work(d).await;
                            });
                            image_export_in_progress = true;
                            continue;
                        }
                        EventRequest::PullImage { image, auth } => {
                            if image_pull_in_progress {
                                continue;
                            }
                            let d = docker.clone();
                            let i = ImagePullWorker::new(image, auth);
                            tx_pull_event = i.1;
                            rx_chunks = i.2;
                            rx_image_pull_results = i.3;
                            tokio::task::spawn(async move {
                                i.0.work(d).await;
                            });
                            image_pull_in_progress = true;
                            continue;
                        }
                        EventRequest::PullImageChunks => {
                            if !image_pull_in_progress {
                                continue;
                            }
                            if let Err(e) = tx_pull_event.send(ImagePullEvent::PollData).await {
                                error!("failed to collect image pull chunks: {}", e);
                                continue;
                            }
                            let chunks = rx_chunks.recv().await.unwrap_or_default();
                            EventResponse::PullImageChunks(chunks)
                        }
                        EventRequest::DockerUriChange { uri } => {
                            if uri == current_uri {
                                continue;
                            }
                            current_uri = uri;
                            docker = match Docker::new(&current_uri)
                                .context("failed to initialize docker")
                            {
                                Ok(docker) => docker,
                                Err(e) => return EventResponse::DockerUriChange(Err(e)),
                            };
                            if image_pull_in_progress {
                                if let Err(e) = tx_pull_event.send(ImagePullEvent::Kill).await {
                                    error!("failed to kill image pull worker: {}", e);
                                }
                            }
                            if image_export_in_progress {
                                if let Err(e) =
                                    _tx_image_export_event.send(ImageExportEvent::Kill).await
                                {
                                    error!("failed to kill image export worker: {}", e);
                                }
                            }

                            if let Some(id) = current_id.as_ref() {
                                if let Err(e) = tx_logs_event.send(LogWorkerEvent::Kill).await {
                                    error!("failed to kill logs worker: {}", e);
                                }
                                if let Err(e) = tx_stats_event.send(StatsWorkerEvent::Kill).await {
                                    error!("failed to kill stats worker: {}", e);
                                }

                                let s = StatsWorker::new(id);
                                tx_stats_event = s.1;
                                rx_stats = s.2;
                                let _ = tokio::spawn(s.0.work(docker.clone()));

                                let w = LogsWorker::new(id);
                                tx_logs_event = w.1;
                                rx_logs = w.2;
                                let _ = tokio::spawn(w.0.work(docker.clone()));
                            }
                            EventResponse::DockerUriChange(Ok(()))
                        }
                        EventRequest::ContainerCreate(opts) => EventResponse::ContainerCreate(
                            docker
                                .containers()
                                .create(&opts)
                                .await
                                .map(|c| c.id().to_string())
                                .context("failed to create a container"),
                        ),
                        EventRequest::SystemInspect => {
                            match docker
                                .version()
                                .await
                                .context("checking docker version failed")
                            {
                                Ok(version) => {
                                    match docker.info().await.context("checking docker info failed")
                                    {
                                        Ok(info) => EventResponse::SystemInspect(Ok(Box::new(
                                            SystemInspectInfo { version, info },
                                        ))),
                                        Err(e) => EventResponse::SystemInspect(Err(e)),
                                    }
                                }
                                Err(e) => EventResponse::SystemInspect(Err(e)),
                            }
                        }
                        EventRequest::SystemDataUsage => {
                            match docker
                                .data_usage()
                                .await
                                .context("checking docker data usage failed")
                            {
                                Ok(usage) => EventResponse::SystemDataUsage(Ok(Box::new(usage))),
                                Err(e) => EventResponse::SystemDataUsage(Err(e)),
                            }
                        }
                        EventRequest::ContainerRename { id, name } => {
                            match docker
                                .containers()
                                .get(&id)
                                .rename(&name)
                                .await
                                .context("renaming container failed")
                            {
                                Ok(_) => EventResponse::ContainerRename(Ok(())),
                                Err(e) => EventResponse::ContainerRename(Err(e)),
                            }
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
