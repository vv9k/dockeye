use docker_api::api::{CpuStats, MemoryStat, MemoryStats, Stats};
use docker_api::Docker;
use futures::StreamExt;
use log::{debug, error, trace};
use std::time::SystemTime;
use tokio::sync::mpsc;

#[derive(Debug, Default, Clone)]
pub struct RunningContainerStats(pub Vec<(std::time::Duration, StatsWrapper)>);

#[derive(Debug, Default, Clone)]
pub struct StatsWrapper {
    pub cpu_usage: f64,
    pub mem_usage: f64,
    pub mem_percent: f64,
    pub mem_limit: f64,
    pub mem_stat: Option<MemoryStat>,
}

impl RunningContainerStats {
    pub fn extend(&mut self, stats: RunningContainerStats) {
        self.0.extend(stats.0.into_iter())
    }
}

fn calculate_mem_usage(stats: Option<&MemoryStats>) -> f64 {
    if let Some(stats) = stats {
        let usage = stats.usage.unwrap_or_default();

        if let Some(stat) = &stats.stats {
            // cgroup v1
            if let Some(v) = stat.total_inactive_file {
                if v < usage {
                    return (usage - v) as f64;
                }
            }

            // cgroup v2
            if let Some(v) = stat.inactive_file {
                if v < usage {
                    return (usage - v) as f64;
                }
            }
        }
    }

    0.
}

fn calculate_mem_percent(used: f64, limit: f64) -> f64 {
    if limit != 0. {
        used / limit as f64 * 100.
    } else {
        0.
    }
}

fn calculate_cpu_percent_usage(stats: Option<&CpuStats>, prev_cpu: u64, prev_sys: u64) -> f64 {
    if let Some(cpu_stats) = stats {
        let cpu_delta = cpu_stats.cpu_usage.total_usage as f64 - prev_cpu as f64;
        let sys_delta = cpu_stats.system_cpu_usage.unwrap_or_default() as f64 - prev_sys as f64;
        let online_cpus = cpu_stats
            .online_cpus
            .and_then(|cpus| {
                if cpus == 0 {
                    cpu_stats.cpu_usage.percpu_usage.as_ref().map(|c| c.len())
                } else {
                    Some(cpus as usize)
                }
            })
            .unwrap_or_default() as f64;

        if sys_delta > 0. && cpu_delta > 0. {
            (cpu_delta / sys_delta) * online_cpus * 100.
        } else {
            0.
        }
    } else {
        0.
    }
}

impl StatsWrapper {
    pub fn from(stats: Stats, prev_cpu: u64, prev_sys: u64) -> Self {
        let cpu_usage = calculate_cpu_percent_usage(stats.cpu_stats.as_ref(), prev_cpu, prev_sys);

        let mem_usage = calculate_mem_usage(stats.memory_stats.as_ref());
        let mem_limit = stats
            .memory_stats
            .as_ref()
            .and_then(|stats| stats.limit.map(|limit| limit as f64))
            .unwrap_or_default();
        let mem_percent = calculate_mem_percent(mem_usage, mem_limit);

        Self {
            cpu_usage,
            mem_usage,
            mem_percent,
            mem_limit,
            mem_stat: stats.memory_stats.and_then(|stats| stats.stats),
        }
    }
}

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
                            Err(e) => {
                                match e {
                                    docker_api::Error::Fault {
                                        code: http::status::StatusCode::NOT_FOUND, message: _
                                    } => break,
                                    e => error!("failed to check container stats: {}", e),
                                }
                            },
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
