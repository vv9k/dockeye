mod image;
mod logs;
mod stats;

use crate::event::{
    ContainerEvent, ContainerEventResponse, EventRequest, EventResponse, ImageEvent,
    ImageEventResponse, ImageInspectInfo, SystemInspectInfo,
};
pub use image::{
    export::{ImageExportEvent, ImageExportWorker},
    import::{ImageImportEvent, ImageImportWorker},
    pull::{ImagePullEvent, ImagePullWorker},
};
pub use logs::{LogWorkerEvent, Logs, LogsWorker};
pub use stats::{RunningContainerStats, StatsWorker, StatsWorkerEvent};

use anyhow::{Context, Result};
use docker_api::{
    api::{ImagePruneOpts, ImagesPruneFilter, RmContainerOpts, RmImageOpts},
    Docker,
};
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
            let (_, mut tx_pull_event, mut rx_pull_chunks, mut rx_image_pull_results) =
                ImagePullWorker::new("".to_string(), None);
            let (_, mut _tx_import_event, mut _rx_import_chunks, mut rx_image_import_results) =
                ImageImportWorker::new("");
            let mut image_export_in_progress = false;
            let mut image_pull_in_progress = false;
            let mut image_import_in_progress = false;

            loop {
                if image_export_in_progress {
                    if let Ok(res) = rx_image_export_results.try_recv() {
                        let rsp = EventResponse::Image(ImageEventResponse::Save(res));
                        let _ = tx_rsp.send(rsp).await;
                        image_export_in_progress = false;
                    }
                }
                if image_pull_in_progress {
                    if let Ok(res) = rx_image_pull_results.try_recv() {
                        let rsp = EventResponse::Image(ImageEventResponse::Pull(res));
                        let _ = tx_rsp.send(rsp).await;
                        if let Some(chunks) = rx_pull_chunks.recv().await {
                            let rsp = EventResponse::Image(ImageEventResponse::PullChunks(chunks));
                            let _ = tx_rsp.send(rsp).await;
                        }
                        image_pull_in_progress = false;
                    }
                }
                if image_import_in_progress {
                    if let Ok(res) = rx_image_import_results.try_recv() {
                        let rsp = EventResponse::Image(ImageEventResponse::Import(res));
                        let _ = tx_rsp.send(rsp).await;
                        //if let Some(_) = _rx_import_chunks.recv().await {}
                        image_pull_in_progress = false;
                    }
                }
                if let Some(req) = inner_rx_req.recv().await {
                    let event_str = format!("{:?}", req);
                    debug!("got request: {}", event_str);
                    let rsp = match req {
                        EventRequest::Container(event) => match event {
                            ContainerEvent::List(opts) => {
                                let opts = opts.unwrap_or_default();
                                match docker.containers().list(&opts).await {
                                    Ok(containers) => EventResponse::Container(
                                        ContainerEventResponse::List(containers),
                                    ),
                                    Err(e) => {
                                        error!("failed to list containers: {}", e);
                                        continue;
                                    }
                                }
                            }
                            ContainerEvent::TraceStart { id } => {
                                if Some(&id) == current_id.as_ref() {
                                    continue;
                                }

                                if current_id.is_some() {
                                    if let Err(e) = tx_logs_event.send(LogWorkerEvent::Kill).await {
                                        error!("failed to send kill event to log worker: {}", e);
                                    }
                                    if let Err(e) =
                                        tx_stats_event.send(StatsWorkerEvent::Kill).await
                                    {
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
                                    Ok(container) => EventResponse::Container(
                                        ContainerEventResponse::Details(Box::new(container)),
                                    ),
                                    Err(e) => {
                                        error!("failed to inspect a container: {}", e);
                                        continue;
                                    }
                                }
                            }
                            ContainerEvent::Details => {
                                if let Some(id) = &current_id {
                                    match docker.containers().get(id).inspect().await {
                                        Ok(container) => EventResponse::Container(
                                            ContainerEventResponse::Details(Box::new(container)),
                                        ),
                                        Err(e) => match e {
                                            docker_api::Error::Fault {
                                                code: http::status::StatusCode::NOT_FOUND,
                                                message: _,
                                            } => EventResponse::Container(
                                                ContainerEventResponse::InspectNotFound,
                                            ),
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
                            ContainerEvent::Delete { id } => {
                                EventResponse::Container(ContainerEventResponse::Delete(
                                    docker
                                        .containers()
                                        .get(&id)
                                        .delete()
                                        .await
                                        .map(|_| id.clone())
                                        .map_err(|e| (id, e)),
                                ))
                            }
                            ContainerEvent::ForceDelete { id } => {
                                EventResponse::Container(ContainerEventResponse::ForceDelete(
                                    docker
                                        .containers()
                                        .get(&id)
                                        .remove(&RmContainerOpts::builder().force(true).build())
                                        .await
                                        .map(|_| id)
                                        .context("force deleting container"),
                                ))
                            }
                            ContainerEvent::Stats => {
                                if let Err(e) =
                                    tx_stats_event.send(StatsWorkerEvent::PollData).await
                                {
                                    error!("failed to collect stats data: {}", e);
                                    continue;
                                }
                                trace!("notified stats worker to poll data, reading stats");
                                if let Some(stats) = rx_stats.recv().await {
                                    trace!("got data {:?}", stats);
                                    EventResponse::Container(ContainerEventResponse::Stats(stats))
                                } else {
                                    log::warn!("no stats available");
                                    continue;
                                }
                            }
                            ContainerEvent::Logs => {
                                if let Err(e) = tx_logs_event.send(LogWorkerEvent::PollData).await {
                                    error!("failed to collect logs: {}", e);
                                    continue;
                                }
                                trace!("notified logs worker to poll data, reading logs");
                                if let Some(logs) = rx_logs.recv().await {
                                    trace!("got data {:?}", logs);
                                    EventResponse::Container(ContainerEventResponse::Logs(logs))
                                } else {
                                    log::warn!("no logs available");
                                    continue;
                                }
                            }
                            ContainerEvent::Pause { id } => {
                                EventResponse::Container(ContainerEventResponse::Pause(
                                    docker
                                        .containers()
                                        .get(id)
                                        .pause()
                                        .await
                                        .context("pausing container"),
                                ))
                            }
                            ContainerEvent::Unpause { id } => {
                                EventResponse::Container(ContainerEventResponse::Unpause(
                                    docker
                                        .containers()
                                        .get(id)
                                        .unpause()
                                        .await
                                        .context("unpausing container"),
                                ))
                            }
                            ContainerEvent::Stop { id } => {
                                EventResponse::Container(ContainerEventResponse::Stop(
                                    docker
                                        .containers()
                                        .get(id)
                                        .stop(Some(Duration::from_millis(0)))
                                        .await
                                        .context("stopping container"),
                                ))
                            }
                            ContainerEvent::Start { id } => {
                                EventResponse::Container(ContainerEventResponse::Start(
                                    docker
                                        .containers()
                                        .get(id)
                                        .start()
                                        .await
                                        .context("starting container"),
                                ))
                            }
                            ContainerEvent::Create(opts) => {
                                EventResponse::Container(ContainerEventResponse::Create(
                                    docker
                                        .containers()
                                        .create(&opts)
                                        .await
                                        .map(|c| c.id().to_string())
                                        .context("failed to create a container"),
                                ))
                            }
                            ContainerEvent::Rename { id, name } => {
                                match docker
                                    .containers()
                                    .get(&id)
                                    .rename(&name)
                                    .await
                                    .context("renaming container failed")
                                {
                                    Ok(_) => EventResponse::Container(
                                        ContainerEventResponse::Rename(Ok(())),
                                    ),
                                    Err(e) => EventResponse::Container(
                                        ContainerEventResponse::Rename(Err(e)),
                                    ),
                                }
                            }
                            ContainerEvent::Prune => {
                                match docker
                                    .containers()
                                    .prune(&Default::default())
                                    .await
                                    .context("pruning containers failed")
                                {
                                    Ok(info) => EventResponse::Container(
                                        ContainerEventResponse::Prune(Ok(info)),
                                    ),
                                    Err(e) => EventResponse::Container(
                                        ContainerEventResponse::Prune(Err(e)),
                                    ),
                                }
                            }
                        },
                        EventRequest::Image(event) => match event {
                            ImageEvent::List(opts) => {
                                let opts = opts.unwrap_or_else(|| {
                                    ImageListOpts::builder().all(true).digests(true).build()
                                });
                                match docker.images().list(&opts).await {
                                    Ok(images) => {
                                        EventResponse::Image(ImageEventResponse::List(images))
                                    }
                                    Err(e) => {
                                        error!("failed to list images: {}", e);
                                        continue;
                                    }
                                }
                            }
                            ImageEvent::Inspect { id } => {
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
                                EventResponse::Image(ImageEventResponse::Inspect(Box::new(
                                    ImageInspectInfo {
                                        details,
                                        distribution_info,
                                        history,
                                    },
                                )))
                            }
                            ImageEvent::Delete { id } => {
                                EventResponse::Image(ImageEventResponse::Delete(
                                    docker.images().get(&id).delete().await.map_err(|e| (id, e)),
                                ))
                            }
                            ImageEvent::ForceDelete { id } => {
                                EventResponse::Image(ImageEventResponse::ForceDelete(
                                    docker
                                        .images()
                                        .get(&id)
                                        .remove(&RmImageOpts::builder().force(true).build())
                                        .await
                                        .context("force deleting image"),
                                ))
                            }
                            ImageEvent::Save { id, output_path } => {
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
                            ImageEvent::Pull { image, auth } => {
                                if image_pull_in_progress {
                                    continue;
                                }
                                let d = docker.clone();
                                let i = ImagePullWorker::new(image, auth);
                                tx_pull_event = i.1;
                                rx_pull_chunks = i.2;
                                rx_image_pull_results = i.3;
                                tokio::task::spawn(async move {
                                    i.0.work(d).await;
                                });
                                image_pull_in_progress = true;
                                continue;
                            }
                            ImageEvent::Import { path } => {
                                if image_import_in_progress {
                                    continue;
                                }
                                let d = docker.clone();
                                let i = ImageImportWorker::new(&path);
                                _tx_import_event = i.1;
                                _rx_import_chunks = i.2;
                                rx_image_import_results = i.3;
                                tokio::task::spawn(async move {
                                    i.0.work(d).await;
                                });
                                image_import_in_progress = true;
                                continue;
                            }
                            ImageEvent::PullChunks => {
                                if !image_pull_in_progress {
                                    continue;
                                }
                                if let Err(e) = tx_pull_event.send(ImagePullEvent::PollData).await {
                                    error!("failed to collect image pull chunks: {}", e);
                                    continue;
                                }
                                let chunks = rx_pull_chunks.recv().await.unwrap_or_default();
                                EventResponse::Image(ImageEventResponse::PullChunks(chunks))
                            }
                            ImageEvent::Search { image } => {
                                match docker
                                    .images()
                                    .search(&image)
                                    .await
                                    .context("image search failed")
                                {
                                    Ok(results) => EventResponse::Image(
                                        ImageEventResponse::Search(Ok(results)),
                                    ),
                                    Err(e) => {
                                        EventResponse::Image(ImageEventResponse::Search(Err(e)))
                                    }
                                }
                            }
                            ImageEvent::Prune => {
                                match docker
                                    .images()
                                    .prune(
                                        &ImagePruneOpts::builder()
                                            .filter([ImagesPruneFilter::Dangling(false)])
                                            .build(),
                                    )
                                    .await
                                    .context("pruning images failed")
                                {
                                    Ok(info) => {
                                        EventResponse::Image(ImageEventResponse::Prune(Ok(info)))
                                    }
                                    Err(e) => {
                                        EventResponse::Image(ImageEventResponse::Prune(Err(e)))
                                    }
                                }
                            }
                        },
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
