use crate::stats::{RunningContainerStats, StatsWrapper};

use docker_api::Docker;
use futures::StreamExt;
use log::{debug, error, trace};
use std::time::SystemTime;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum StatsWorkerEvent {
    PollData,
    Kill,
}

#[derive(Debug)]
pub struct StatsWorker {
    pub rx_events: mpsc::Receiver<StatsWorkerEvent>,
    pub tx_stats: mpsc::Sender<Box<RunningContainerStats>>,
    pub current_id: String,
    pub prev_cpu: u64,
    pub prev_sys: u64,
    pub stats: Box<RunningContainerStats>,
    pub timer: SystemTime,
}

impl StatsWorker {
    pub fn new(
        id: impl Into<String>,
    ) -> (
        Self,
        mpsc::Sender<StatsWorkerEvent>,
        mpsc::Receiver<Box<RunningContainerStats>>,
    ) {
        let (tx_stats, rx_stats) = mpsc::channel::<Box<RunningContainerStats>>(128);
        let (tx_events, rx_events) = mpsc::channel::<StatsWorkerEvent>(128);

        (
            Self {
                rx_events,
                tx_stats,
                current_id: id.into(),
                prev_cpu: 0,
                prev_sys: 0,
                stats: Box::new(RunningContainerStats(vec![])),
                timer: SystemTime::now(),
            },
            tx_events,
            rx_stats,
        )
    }

    async fn send_stats(&mut self) {
        debug!("got poll data request, sending info");
        if let Err(e) = self.tx_stats.send(std::mem::take(&mut self.stats)).await {
            error!("failed to send container info: {}", e);
        }
    }

    pub async fn work(mut self, docker: Docker) {
        let container = docker.containers().get(&self.current_id);
        let mut stats = container.stats();
        loop {
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
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        },
                    }
                }
                event = self.rx_events.recv() => {
                    match event {
                        Some(StatsWorkerEvent::PollData) => self.send_stats().await,
                        Some(StatsWorkerEvent::Kill) => break,
                        None => continue,
                    }
                },
            }
        }
    }
}
