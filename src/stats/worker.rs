use crate::stats::{RunningContainerStats, StatsWrapper};

use anyhow::Result;
use docker_api::{Container, Docker};
use futures::StreamExt;
use log::{debug, error, trace};
use std::time::SystemTime;
use tokio::sync::mpsc;

pub struct StatsWorker<'d> {
    pub rx_id: mpsc::Receiver<String>,
    pub rx_want_data: mpsc::Receiver<()>,
    pub tx_stats: mpsc::Sender<Box<RunningContainerStats>>,
    pub current_id: Option<String>,
    pub container: Option<Container<'d>>,
    pub prev_cpu: u64,
    pub prev_sys: u64,
    pub stats: Box<RunningContainerStats>,
    pub timer: SystemTime,
}

impl<'d> StatsWorker<'d> {
    pub fn new() -> (
        Self,
        mpsc::Sender<String>,
        mpsc::Sender<()>,
        mpsc::Receiver<Box<RunningContainerStats>>,
    ) {
        let (tx_stats, rx_stats) = mpsc::channel::<Box<RunningContainerStats>>(128);
        let (tx_want_data, rx_want_data) = mpsc::channel::<()>(128);
        let (tx_id, rx_id) = mpsc::channel::<String>(128);

        (
            Self {
                rx_id,
                rx_want_data,
                tx_stats,
                current_id: None,
                container: None,
                prev_cpu: 0,
                prev_sys: 0,
                stats: Box::new(RunningContainerStats(vec![])),
                timer: SystemTime::now(),
            },
            tx_id,
            tx_want_data,
            rx_stats,
        )
    }

    fn set_container(&mut self, docker: &'d Docker, id: &str) {
        if Some(id) != self.current_id.as_deref() {
            debug!("changing container to {}", id);
            self.current_id = Some(id.to_string());
            self.container = Some(docker.containers().get(id));
            self.stats.0.clear();
            self.prev_cpu = 0;
            self.prev_sys = 0;
            self.timer = SystemTime::now();
        }
    }

    async fn send_stats(&mut self) {
        debug!("got poll data request, sending info");
        if let Err(e) = self.tx_stats.send(self.stats.clone()).await {
            error!("failed to send container info: {}", e);
        }
    }

    async fn inner_work(&mut self, docker: &'d Docker, id: Option<String>) {
        if let Some(id) = id {
            self.set_container(&docker, &id);
            loop {
                let container = self.container.as_ref().unwrap();
                let mut stats = container.stats();

                tokio::select! {
                    data = stats.next() => {
                        match data {
                            Some(data) => match data {
                                Ok(data) => {
                                    trace!("adding datapoint");
                                    let (_cpu, _sys) = data.precpu_stats.as_ref()
                                        .map(|data|
                                            (data.cpu_usage.total_usage, data.system_cpu_usage.unwrap_or_default())
                                    ).unwrap_or_default();

                                    self.stats.0.push(
                                        (
                                            self.timer.elapsed().unwrap_or_default(),
                                            StatsWrapper::from(data, self.prev_cpu, self.prev_sys)
                                        )
                                    );
                                    self.prev_cpu = _cpu;
                                    self.prev_sys = _sys;
                                }
                                Err(e) => error!("failed to check container stats: {}", e),
                            },
                            None => {
                                log::trace!("no container stats available");
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            },
                        }
                    }
                    _ = self.rx_want_data.recv() => self.send_stats().await,
                    _id = self.rx_id.recv() => if let Some(_id) = _id {
                        self.set_container(&docker, &_id);
                    }
                }
            }
        }
    }

    pub async fn work(mut self, docker: Docker) -> Result<()> {
        loop {
            tokio::select! {
                id = self.rx_id.recv() => self.inner_work(&docker, id).await,
                _ = self.rx_want_data.recv() => self.send_stats().await
            }
        }
    }
}
