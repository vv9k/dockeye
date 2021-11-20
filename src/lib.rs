mod app;
pub use app::App;

use anyhow::Result;
use docker_api::api::{
    ContainerDetails, ContainerInfo, ContainerListOpts, ImageInfo, ImageListOpts, Stats,
};
use docker_api::Docker;
use futures::StreamExt;
use log::{debug, error, trace};
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy)]
pub struct StatsWrapper {
    cpu_usage: f64,
    mem_usage: f64,
}

impl From<Stats> for StatsWrapper {
    fn from(stats: Stats) -> Self {
        Self {
            cpu_usage: stats
                .cpu_stats
                .map(|s| s.cpu_usage.total_usage as f64)
                .unwrap_or_default(),
            mem_usage: 0.,
        }
    }
}

#[derive(Debug)]
pub enum EventRequest {
    ListContainers(Option<ContainerListOpts>),
    ListImages(Option<ImageListOpts>),
    InspectContainer { id: String },
    DeleteContainer { id: String },
    ContainerStatsStart { id: String },
    ContainerStats,
}

#[derive(Debug)]
pub enum EventResponse {
    ListContainers(Vec<ContainerInfo>),
    ListImages(Vec<ImageInfo>),
    InspectContainer(ContainerDetails),
    ContainerStats(Vec<(Duration, StatsWrapper)>),
    DeleteContainer(String),
}

pub struct StatsWorker {
    docker: Docker,
    rx_id: mpsc::Receiver<String>,
    rx_want_data: mpsc::Receiver<()>,
    tx_stats: mpsc::Sender<Vec<(Duration, StatsWrapper)>>,
}

impl StatsWorker {
    pub async fn work(self) -> Result<()> {
        let Self {
            mut rx_id,
            tx_stats,
            mut rx_want_data,
            docker,
        } = self;
        loop {
            let mut current_id = None;
            let mut datapoints = vec![];
            if let Some(mut id) = rx_id.recv().await {
                let mut start = SystemTime::now();
                let container = docker.containers().get(&id);
                let mut stats = container.stats();
                loop {
                    tokio::select! {
                        data = stats.next() => {
                            match data {
                                Some(stats) => match stats {
                                    Ok(stats) => {
                                        if Some(&id) != current_id.as_ref() {
                                            current_id = Some(id.to_string());
                                            datapoints.clear();
                                            start = SystemTime::now();
                                        }
                                        datapoints.push((start.elapsed().unwrap_or_default(), StatsWrapper::from(stats)));
                                    }
                                    Err(e) => {
                                        error!("failed to check container stats: {}", e);
                                        continue;
                                    }
                                },
                                None => {
                                    error!("no container stats available");
                                    continue;
                                }
                            }
                        }
                        _ = rx_want_data.recv() => {
                            tx_stats.send(datapoints.clone()).await;
                        }
                        _id = rx_id.recv() => {
                            if let Some(_id) = _id {
                                if _id != id {
                                    id = _id;
                                    datapoints.clear();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
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
                        EventRequest::InspectContainer { id } => {
                            match docker.containers().get(id).inspect().await {
                                Ok(container) => EventResponse::InspectContainer(container),
                                Err(e) => {
                                    error!("failed to inspect a container: {}", e);
                                    continue;
                                }
                            }
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
                        EventRequest::ContainerStatsStart { id } => {
                            if let Err(e) = tx_id.send(id).await {
                                error!("failed to start stats collection: {}", e);
                            }
                            continue;
                        }
                        EventRequest::ContainerStats => {
                            if let Err(e) = tx_want_data.send(()).await {
                                error!("failed to collect stats data: {}", e);
                                continue;
                            }
                            if let Some(stats) = rx_stats.recv().await {
                                EventResponse::ContainerStats(stats)
                            } else {
                                error!("no stats available");
                                continue;
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
