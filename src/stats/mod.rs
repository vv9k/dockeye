pub mod worker;

use docker_api::api::{CpuStats, MemoryStat, MemoryStats, Stats};

#[derive(Debug, Clone)]
pub struct RunningContainerStats(pub Vec<(std::time::Duration, StatsWrapper)>);

#[derive(Debug, Clone)]
pub struct StatsWrapper {
    pub cpu_usage: f64,
    pub mem_usage: f64,
    pub mem_percent: f64,
    pub mem_limit: f64,
    pub mem_stat: Option<MemoryStat>,
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
