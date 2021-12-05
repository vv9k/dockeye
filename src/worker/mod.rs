mod events;
mod image;
mod logs;
mod stats;

use crate::event::{
    ContainerEvent, ContainerEventResponse, EventRequest, EventResponse, ImageEvent,
    ImageEventResponse, ImageInspectInfo, SystemInspectInfo,
};
pub use events::EventsWorker;
pub use image::{export::ImageExportWorker, import::ImageImportWorker, pull::ImagePullWorker};
pub use logs::{Logs, LogsWorker};
pub use stats::{RunningContainerStats, StatsWorker};

use anyhow::{anyhow, Context, Result};
use docker_api::{
    api::{
        ClearCacheOpts, ContainerId, Event, ImageBuildChunk, ImageId, ImageListOpts,
        ImagePruneOpts, ImagesPruneFilter, RmContainerOpts, RmImageOpts,
    },
    Docker,
};
use log::{debug, error, trace};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, PartialEq)]
pub enum WorkerEvent {
    PollData,
    Kill,
}

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
            docker,
            uri,
            rx_req,
            tx_rsp,
        } = self;
        // used to transfer requests between the listener task and the worker task
        let (inner_tx_req, inner_rx_req) = mpsc::channel::<EventRequest>(64);

        let listener = tokio::spawn(listener_task(rx_req, inner_tx_req));

        let worker = tokio::spawn(inner_work(docker, uri, inner_rx_req, tx_rsp));
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

/// Task responsible for receiving events from gui and passing them to the worker task
async fn listener_task(
    mut rx_req: mpsc::Receiver<EventRequest>,
    inner_tx_req: mpsc::Sender<EventRequest>,
) {
    loop {
        if let Some(req) = rx_req.recv().await {
            debug!("[listener] got request: {:?}", req);
            if let Err(e) = inner_tx_req.send(req).await {
                error!("[listener] failed to send request to worker task: {}", e);
            }
        }
    }
}

async fn inner_work(
    mut docker: Docker,
    current_uri: String,
    mut inner_rx_req: mpsc::Receiver<EventRequest>,
    tx_rsp: mpsc::Sender<EventResponse>,
) {
    if let Err(e) = docker.adjust_api_version().await {
        error!("failed to adjust docker API version: {}", e);
    }

    let (sys_events_worker, tx_sys_events_event, rx_sys_events) = EventsWorker::new();
    let d = docker.clone();
    tokio::task::spawn(async move {
        sys_events_worker.work(d).await;
    });

    let mut workers = WorkerHandles {
        current_uri,
        tx_rsp,
        tx_sys_events_event,
        rx_sys_events,
        ..Default::default()
    };

    loop {
        check_image_in_progress_events(&mut workers).await;

        if let Some(req) = inner_rx_req.recv().await {
            let event_str = format!("{:?}", req);
            debug!("got request: {}", event_str);
            if let Some(rsp) = handle_event(&mut docker, req, &mut workers).await {
                debug!("sending response to event: {}", event_str);
                //trace!("{:?}", rsp);

                let _ = workers.tx_rsp.send(rsp).await;
            }
        }
    }
}

struct WorkerHandles {
    current_uri: String,
    containers: ContainerWorkerHandles,
    images: ImageWorkerHandles,
    tx_sys_events_event: mpsc::Sender<WorkerEvent>,
    rx_sys_events: mpsc::Receiver<Vec<Event>>,
    tx_rsp: mpsc::Sender<EventResponse>,
}

impl Default for WorkerHandles {
    fn default() -> Self {
        Self {
            current_uri: String::new(),
            containers: ContainerWorkerHandles::default(),
            images: ImageWorkerHandles::default(),
            tx_sys_events_event: mpsc::channel::<WorkerEvent>(1).0,
            rx_sys_events: mpsc::channel::<Vec<Event>>(1).1,
            tx_rsp: mpsc::channel::<EventResponse>(1).0,
        }
    }
}

struct ContainerWorkerHandles {
    current_id: Option<ContainerId>,
    tx_logs_event: mpsc::Sender<WorkerEvent>,
    rx_logs: mpsc::Receiver<Box<Logs>>,
    tx_stats_event: mpsc::Sender<WorkerEvent>,
    rx_stats: mpsc::Receiver<Box<RunningContainerStats>>,
}

impl Default for ContainerWorkerHandles {
    fn default() -> Self {
        Self {
            current_id: None,
            tx_logs_event: mpsc::channel::<WorkerEvent>(1).0,
            rx_logs: mpsc::channel::<Box<Logs>>(1).1,
            tx_stats_event: mpsc::channel::<WorkerEvent>(1).0,
            rx_stats: mpsc::channel::<Box<RunningContainerStats>>(1).1,
        }
    }
}

struct ImageWorkerHandles {
    pull_in_progress: bool,
    export_in_progress: bool,
    import_in_progress: bool,
    tx_pull_event: mpsc::Sender<WorkerEvent>,
    rx_pull_chunks: mpsc::Receiver<Vec<ImageBuildChunk>>,
    rx_pull_results: mpsc::Receiver<anyhow::Result<ImageId>>,
    tx_export_event: mpsc::Sender<WorkerEvent>,
    rx_export_results: mpsc::Receiver<anyhow::Result<(ImageId, std::path::PathBuf)>>,
    tx_import_event: mpsc::Sender<WorkerEvent>,
    rx_import_chunks: mpsc::Receiver<Vec<ImageBuildChunk>>,
    rx_import_results: mpsc::Receiver<anyhow::Result<ImageId>>,
}

impl Default for ImageWorkerHandles {
    fn default() -> Self {
        Self {
            pull_in_progress: false,
            export_in_progress: false,
            import_in_progress: false,
            tx_pull_event: mpsc::channel::<WorkerEvent>(1).0,
            rx_pull_chunks: mpsc::channel::<Vec<ImageBuildChunk>>(1).1,
            rx_pull_results: mpsc::channel::<anyhow::Result<ImageId>>(1).1,
            tx_export_event: mpsc::channel::<WorkerEvent>(1).0,
            rx_export_results: mpsc::channel::<anyhow::Result<(ImageId, std::path::PathBuf)>>(1).1,
            tx_import_event: mpsc::channel::<WorkerEvent>(1).0,
            rx_import_chunks: mpsc::channel::<Vec<ImageBuildChunk>>(1).1,
            rx_import_results: mpsc::channel::<anyhow::Result<ImageId>>(1).1,
        }
    }
}

async fn handle_event(
    docker: &mut Docker,
    req: EventRequest,
    workers: &mut WorkerHandles,
) -> Option<EventResponse> {
    match req {
        EventRequest::Container(event) => {
            match handle_container_event(
                docker,
                event,
                &mut workers.tx_rsp,
                &mut workers.containers,
            )
            .await
            {
                Ok(Some(rsp)) => Some(rsp),
                Ok(None) => None,
                Err(e) => {
                    error!("{}", e);
                    None
                }
            }
        }
        EventRequest::Image(event) => {
            match handle_image_event(docker, event, &mut workers.images).await {
                Ok(Some(rsp)) => Some(rsp),
                Ok(None) => None,
                Err(e) => {
                    error!("{}", e);
                    None
                }
            }
        }
        EventRequest::DockerUriChange { uri } => {
            if uri == workers.current_uri {
                return None;
            }
            workers.current_uri = uri;
            *docker = match Docker::new(&workers.current_uri).context("failed to initialize docker")
            {
                Ok(d) => d,
                Err(e) => return Some(EventResponse::DockerUriChange(Err(e))),
            };
            if workers.images.pull_in_progress {
                if let Err(e) = workers.images.tx_pull_event.send(WorkerEvent::Kill).await {
                    error!("failed to kill image pull worker: {}", e);
                }
            }
            if workers.images.export_in_progress {
                if let Err(e) = workers.images.tx_export_event.send(WorkerEvent::Kill).await {
                    error!("failed to kill image export worker: {}", e);
                }
            }

            if let Some(id) = workers.containers.current_id.as_ref() {
                if let Err(e) = workers
                    .containers
                    .tx_logs_event
                    .send(WorkerEvent::Kill)
                    .await
                {
                    error!("failed to kill logs worker: {}", e);
                }
                if let Err(e) = workers
                    .containers
                    .tx_stats_event
                    .send(WorkerEvent::Kill)
                    .await
                {
                    error!("failed to kill stats worker: {}", e);
                }

                let s = StatsWorker::new(id);
                workers.containers.tx_stats_event = s.1;
                workers.containers.rx_stats = s.2;
                let _ = tokio::spawn(s.0.work(docker.clone()));

                let w = LogsWorker::new(id);
                workers.containers.tx_logs_event = w.1;
                workers.containers.rx_logs = w.2;
                let _ = tokio::spawn(w.0.work(docker.clone()));
            }
            Some(EventResponse::DockerUriChange(Ok(())))
        }
        EventRequest::SystemInspect => {
            match docker
                .version()
                .await
                .context("checking docker version failed")
            {
                Ok(version) => match docker.info().await.context("checking docker info failed") {
                    Ok(info) => Some(EventResponse::SystemInspect(Ok(Box::new(
                        SystemInspectInfo { version, info },
                    )))),
                    Err(e) => Some(EventResponse::SystemInspect(Err(e))),
                },
                Err(e) => Some(EventResponse::SystemInspect(Err(e))),
            }
        }
        EventRequest::SystemDataUsage => {
            match docker
                .data_usage()
                .await
                .context("checking docker data usage failed")
            {
                Ok(usage) => Some(EventResponse::SystemDataUsage(Ok(Box::new(usage)))),
                Err(e) => Some(EventResponse::SystemDataUsage(Err(e))),
            }
        }
        EventRequest::NotifyGui(event) => Some(EventResponse::NotifyGui(event.into())),
        EventRequest::SystemEvents => {
            if let Err(e) = workers
                .tx_sys_events_event
                .send(WorkerEvent::PollData)
                .await
            {
                error!("failed to collect system events: {}", e);
                return None;
            }
            let events = workers.rx_sys_events.recv().await.unwrap_or_default();
            Some(EventResponse::SystemEvents(events))
        }
    }
}

async fn handle_container_event(
    docker: &Docker,
    event: ContainerEvent,
    tx_rsp: &mut mpsc::Sender<EventResponse>,
    container_workers: &mut ContainerWorkerHandles,
) -> Result<Option<EventResponse>> {
    match event {
        ContainerEvent::List(opts) => {
            let opts = opts.unwrap_or_default();
            match docker.containers().list(&opts).await {
                Ok(containers) => Ok(Some(EventResponse::Container(
                    ContainerEventResponse::List(containers),
                ))),
                Err(e) => Err(anyhow!("failed to list containers: {}", e)),
            }
        }
        ContainerEvent::TraceStart { id } => {
            if Some(&id) == container_workers.current_id.as_ref() {
                return Ok(None);
            }

            if container_workers.current_id.is_some() {
                if let Err(e) = container_workers
                    .tx_logs_event
                    .send(WorkerEvent::Kill)
                    .await
                {
                    error!("failed to send kill event to log worker: {}", e);
                }
                if let Err(e) = container_workers
                    .tx_stats_event
                    .send(WorkerEvent::Kill)
                    .await
                {
                    error!("failed to send kill event to stats worker: {}", e);
                }
            }

            container_workers.current_id = Some(id.clone());

            let s = StatsWorker::new(&id);
            container_workers.tx_stats_event = s.1;
            container_workers.rx_stats = s.2;
            let _ = tokio::spawn(s.0.work(docker.clone()));

            let w = LogsWorker::new(&id);
            container_workers.tx_logs_event = w.1;
            container_workers.rx_logs = w.2;
            let _ = tokio::spawn(w.0.work(docker.clone()));

            match docker.containers().get(&id).inspect().await {
                Ok(container) => Ok(Some(EventResponse::Container(
                    ContainerEventResponse::Details(Box::new(container)),
                ))),
                Err(e) => Err(anyhow!("failed to inspect a container: {}", e)),
            }
        }
        ContainerEvent::Details => {
            if let Some(id) = &container_workers.current_id {
                match docker.containers().get(id).inspect().await {
                    Ok(container) => Ok(Some(EventResponse::Container(
                        ContainerEventResponse::Details(Box::new(container)),
                    ))),
                    Err(e) => match e {
                        docker_api::Error::Fault {
                            code: http::status::StatusCode::NOT_FOUND,
                            message: _,
                        } => Ok(Some(EventResponse::Container(
                            ContainerEventResponse::InspectNotFound,
                        ))),
                        e => Err(anyhow!("failed to inspect container: {}", e)),
                    },
                }
            } else {
                Err(anyhow!("failed to inspect a container: no current id set"))
            }
        }
        ContainerEvent::Delete { id } => Ok(Some(EventResponse::Container(
            ContainerEventResponse::Delete(
                docker
                    .containers()
                    .get(&id)
                    .delete()
                    .await
                    .map(|_| id.clone())
                    .map_err(|e| (id, e)),
            ),
        ))),
        ContainerEvent::ForceDelete { id } => Ok(Some(EventResponse::Container(
            ContainerEventResponse::ForceDelete(
                docker
                    .containers()
                    .get(&id)
                    .remove(&RmContainerOpts::builder().force(true).build())
                    .await
                    .map(|_| id)
                    .context("force deleting container"),
            ),
        ))),
        ContainerEvent::Stats => {
            if let Err(e) = container_workers
                .tx_stats_event
                .send(WorkerEvent::PollData)
                .await
            {
                return Err(anyhow!("failed to collect stats data: {}", e));
            }
            trace!("notified stats worker to poll data, reading stats");
            if let Some(stats) = container_workers.rx_stats.recv().await {
                trace!("got data {:?}", stats);
                Ok(Some(EventResponse::Container(
                    ContainerEventResponse::Stats(stats),
                )))
            } else {
                log::warn!("no stats available");
                Ok(None)
            }
        }
        ContainerEvent::Logs => {
            if let Err(e) = container_workers
                .tx_logs_event
                .send(WorkerEvent::PollData)
                .await
            {
                return Err(anyhow!("failed to collect logs: {}", e));
            }
            trace!("notified logs worker to poll data, reading logs");
            if let Some(logs) = container_workers.rx_logs.recv().await {
                trace!("got data {:?}", logs);
                Ok(Some(EventResponse::Container(
                    ContainerEventResponse::Logs(logs),
                )))
            } else {
                log::warn!("no logs available");
                Ok(None)
            }
        }
        ContainerEvent::Pause { id } => Ok(Some(EventResponse::Container(
            ContainerEventResponse::Pause(
                docker
                    .containers()
                    .get(id)
                    .pause()
                    .await
                    .context("pausing container"),
            ),
        ))),
        ContainerEvent::Unpause { id } => Ok(Some(EventResponse::Container(
            ContainerEventResponse::Unpause(
                docker
                    .containers()
                    .get(id)
                    .unpause()
                    .await
                    .context("unpausing container"),
            ),
        ))),
        ContainerEvent::Stop { id } => Ok(Some(EventResponse::Container(
            ContainerEventResponse::Stop(
                docker
                    .containers()
                    .get(id)
                    .stop(Some(Duration::from_millis(0)))
                    .await
                    .context("stopping container"),
            ),
        ))),
        ContainerEvent::Start { id } => Ok(Some(EventResponse::Container(
            ContainerEventResponse::Start(
                docker
                    .containers()
                    .get(id)
                    .start()
                    .await
                    .context("starting container"),
            ),
        ))),
        ContainerEvent::Create(opts) => Ok(Some(EventResponse::Container(
            ContainerEventResponse::Create(
                docker
                    .containers()
                    .create(&opts)
                    .await
                    .map(|c| c.id().to_string())
                    .context("failed to create a container"),
            ),
        ))),
        ContainerEvent::Rename { id, name } => {
            match docker
                .containers()
                .get(&id)
                .rename(&name)
                .await
                .context("renaming container failed")
            {
                Ok(_) => Ok(Some(EventResponse::Container(
                    ContainerEventResponse::Rename(Ok(())),
                ))),
                Err(e) => Ok(Some(EventResponse::Container(
                    ContainerEventResponse::Rename(Err(e)),
                ))),
            }
        }
        ContainerEvent::Prune => {
            match docker
                .containers()
                .prune(&Default::default())
                .await
                .context("pruning containers failed")
            {
                Ok(info) => Ok(Some(EventResponse::Container(
                    ContainerEventResponse::Prune(Ok(info)),
                ))),
                Err(e) => Ok(Some(EventResponse::Container(
                    ContainerEventResponse::Prune(Err(e)),
                ))),
            }
        }
        ContainerEvent::Restart { id } => {
            let _ = tx_rsp
                .send(EventResponse::Container(
                    ContainerEventResponse::RestartInProgress { id: id.clone() },
                ))
                .await;
            match docker
                .containers()
                .get(&id)
                .restart(None)
                .await
                .map(|_| id)
                .context("restarting container failed")
            {
                Ok(id) => Ok(Some(EventResponse::Container(
                    ContainerEventResponse::Restart(Ok(id)),
                ))),
                Err(e) => Ok(Some(EventResponse::Container(
                    ContainerEventResponse::Restart(Err(e)),
                ))),
            }
        }
        ContainerEvent::ProcessList => {
            if let Some(id) = container_workers.current_id.as_ref() {
                match docker
                    .containers()
                    .get(id)
                    .top(Some("-aux"))
                    .await
                    .context("listing container processes failed")
                {
                    Ok(top) => Ok(Some(EventResponse::Container(
                        ContainerEventResponse::ProcessList(Ok(top)),
                    ))),
                    Err(e) => Ok(Some(EventResponse::Container(
                        ContainerEventResponse::ProcessList(Err(e)),
                    ))),
                }
            } else {
                Ok(None)
            }
        }
    }
}

async fn handle_image_event(
    docker: &Docker,
    event: ImageEvent,
    workers: &mut ImageWorkerHandles,
) -> Result<Option<EventResponse>> {
    match event {
        ImageEvent::List(opts) => {
            let opts =
                opts.unwrap_or_else(|| ImageListOpts::builder().all(true).digests(true).build());
            match docker.images().list(&opts).await {
                Ok(images) => Ok(Some(EventResponse::Image(ImageEventResponse::List(images)))),
                Err(e) => Err(anyhow!("failed to list images: {}", e)),
            }
        }
        ImageEvent::Inspect { id } => {
            let image = docker.images().get(id);
            let details = match image.inspect().await {
                Ok(details) => details,
                Err(e) => {
                    return Err(anyhow!("failed to inspect an image: {}", e));
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
                    return Err(anyhow!("failed to check image history: {}", e));
                }
            };
            Ok(Some(EventResponse::Image(ImageEventResponse::Inspect(
                Box::new(ImageInspectInfo {
                    details,
                    distribution_info,
                    history,
                }),
            ))))
        }
        ImageEvent::Delete { id } => Ok(Some(EventResponse::Image(ImageEventResponse::Delete(
            docker.images().get(&id).delete().await.map_err(|e| (id, e)),
        )))),
        ImageEvent::ForceDelete { id } => {
            Ok(Some(EventResponse::Image(ImageEventResponse::ForceDelete(
                docker
                    .images()
                    .get(&id)
                    .remove(&RmImageOpts::builder().force(true).build())
                    .await
                    .context("force deleting image"),
            ))))
        }
        ImageEvent::Save { id, output_path } => {
            let d = docker.clone();
            let i = ImageExportWorker::new(id, output_path);
            workers.tx_export_event = i.1;
            workers.rx_export_results = i.2;
            tokio::task::spawn(async move {
                i.0.work(d).await;
            });
            workers.export_in_progress = true;
            Ok(None)
        }
        ImageEvent::Pull { image, auth } => {
            if workers.pull_in_progress {
                return Ok(None);
            }
            let d = docker.clone();
            let i = ImagePullWorker::new(image, auth);
            workers.tx_pull_event = i.1;
            workers.rx_pull_chunks = i.2;
            workers.rx_pull_results = i.3;
            tokio::task::spawn(async move {
                i.0.work(d).await;
            });
            workers.pull_in_progress = true;
            Ok(None)
        }
        ImageEvent::Import { path } => {
            if workers.import_in_progress {
                return Ok(None);
            }
            let d = docker.clone();
            let i = ImageImportWorker::new(&path);
            workers.tx_import_event = i.1;
            workers.rx_import_chunks = i.2;
            workers.rx_import_results = i.3;
            tokio::task::spawn(async move {
                i.0.work(d).await;
            });
            workers.import_in_progress = true;
            Ok(None)
        }
        ImageEvent::PullChunks => {
            if !workers.pull_in_progress {
                return Ok(None);
            }
            if let Err(e) = workers.tx_pull_event.send(WorkerEvent::PollData).await {
                return Err(anyhow!("failed to collect image import chunks: {}", e));
            }
            let chunks = workers.rx_pull_chunks.recv().await.unwrap_or_default();
            Ok(Some(EventResponse::Image(ImageEventResponse::PullChunks(
                chunks,
            ))))
        }
        ImageEvent::Search { image } => {
            match docker
                .images()
                .search(&image)
                .await
                .context("image search failed")
            {
                Ok(results) => Ok(Some(EventResponse::Image(ImageEventResponse::Search(Ok(
                    results,
                ))))),
                Err(e) => Ok(Some(EventResponse::Image(ImageEventResponse::Search(Err(
                    e,
                ))))),
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
                Ok(info) => Ok(Some(EventResponse::Image(ImageEventResponse::Prune(Ok(
                    info,
                ))))),
                Err(e) => Ok(Some(EventResponse::Image(ImageEventResponse::Prune(Err(
                    e,
                ))))),
            }
        }
        ImageEvent::ClearCache => {
            match docker
                .images()
                .clear_cache(&ClearCacheOpts::builder().all(true).build())
                .await
                .context("clearing image cache failed")
            {
                Ok(info) => Ok(Some(EventResponse::Image(ImageEventResponse::ClearCache(
                    Ok(info),
                )))),
                Err(e) => Ok(Some(EventResponse::Image(ImageEventResponse::ClearCache(
                    Err(e),
                )))),
            }
        }
        ImageEvent::Tag { id, opts } => {
            match docker
                .images()
                .get(&id)
                .tag(&opts)
                .await
                .context("tagging image failed")
            {
                Ok(_) => Ok(Some(EventResponse::Image(ImageEventResponse::Tag(Ok(()))))),
                Err(e) => Ok(Some(EventResponse::Image(ImageEventResponse::Tag(Err(e))))),
            }
        }
    }
}

async fn check_image_in_progress_events(workers: &mut WorkerHandles) {
    if workers.images.export_in_progress {
        if let Ok(res) = workers.images.rx_export_results.try_recv() {
            let rsp = EventResponse::Image(ImageEventResponse::Save(res));
            let _ = workers.tx_rsp.send(rsp).await;
            workers.images.export_in_progress = false;
        }
    }
    if workers.images.pull_in_progress {
        if let Ok(res) = workers.images.rx_pull_results.try_recv() {
            let rsp = EventResponse::Image(ImageEventResponse::Pull(res));
            let _ = workers.tx_rsp.send(rsp).await;
            if let Some(chunks) = workers.images.rx_pull_chunks.recv().await {
                let rsp = EventResponse::Image(ImageEventResponse::PullChunks(chunks));
                let _ = workers.tx_rsp.send(rsp).await;
            }
            workers.images.pull_in_progress = false;
        }
    }
    if workers.images.import_in_progress {
        if let Ok(res) = workers.images.rx_import_results.try_recv() {
            let rsp = EventResponse::Image(ImageEventResponse::Import(res));
            let _ = workers.tx_rsp.send(rsp).await;
            //if let Some(_) = _rx_import_chunks.recv().await {}
            workers.images.pull_in_progress = false;
        }
    }
}
