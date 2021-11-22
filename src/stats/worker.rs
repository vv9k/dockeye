use crate::stats::{RunningContainerStats, StatsWrapper};

use anyhow::Result;
use docker_api::Docker;
use futures::StreamExt;
use log::{debug, error, trace};
use std::time::SystemTime;
use tokio::sync::mpsc;

pub struct StatsWorker {
    pub docker: Docker,
    pub rx_id: mpsc::Receiver<String>,
    pub rx_want_data: mpsc::Receiver<()>,
    pub tx_stats: mpsc::Sender<Box<RunningContainerStats>>,
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
            let mut info = Box::new(RunningContainerStats(vec![]));
            let mut prev_cpu = 0;
            let mut prev_sys = 0;

            tokio::select! {
                id = rx_id.recv() => {
                    if let Some(id) = id {
                        if Some(&id) != current_id.as_ref() {
                            current_id = Some(id.to_string());
                            info.0.clear();
                        }
                        let mut start = SystemTime::now();
                        let container = docker.containers().get(&id);
                        let mut stats = container.stats();
                        loop {
                            tokio::select! {
                                data = stats.next() => {
                                    match data {
                                        Some(stats) => match stats {
                                            Ok(stats) => {
                                                trace!("[stats-worker] adding datapoint");
                                                let (_cpu, _sys) = stats.precpu_stats.as_ref()
                                                    .map(|stats|
                                                        (stats.cpu_usage.total_usage, stats.system_cpu_usage.unwrap_or_default())
                                                ).unwrap_or_default();

                                                info.0.push(
                                                    (
                                                        start.elapsed().unwrap_or_default(),
                                                        StatsWrapper::from(stats, prev_cpu, prev_sys)
                                                    )
                                                );
                                                prev_cpu = _cpu;
                                                prev_sys = _sys;
                                            }
                                            Err(e) => error!("[stats-worker] failed to check container stats: {}", e),
                                        },
                                        None => error!("[stats-worker] no container stats available"),
                                    }
                                }
                                _ = rx_want_data.recv() => {
                                    debug!("[stats-worker] got poll data request, sending info");
                                    if let Err(e) = tx_stats.send(info.clone()).await {
                                        error!("[stats-worker] failed to send container info: {}", e);
                                    }
                                }
                                _id = rx_id.recv() => if let Some(_id) = _id {
                                    if Some(&_id) != current_id.as_ref() {
                                        debug!("[stats-worker] received new id: {}", _id);
                                        current_id = Some(_id);
                                        info.0.clear();
                                        start = SystemTime::now();
                                    }
                                }
                            }
                        }
                    }
                }
                _ = rx_want_data.recv() => {
                    debug!("[stats-worker] got poll data request, sending info");
                    if let Err(e) = tx_stats.send(info.clone()).await {
                        error!("[stats-worker] failed to send container info: {}", e);
                    }
                }
            }
        }
    }
}
