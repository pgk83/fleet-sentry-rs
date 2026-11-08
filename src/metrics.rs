use std::fs;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct HostMetrics {
    pub os_name: String,
    pub uptime_secs: u64,
    pub cpu_percent: f64,
    pub ram_percent: f64,
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
    pub disk_percent: f64,
    pub disk_worst_path: String,
}

pub struct MetricsCollector {
    prev_idle: u64,
    prev_total: u64,
    monitored_paths: Vec<String>,
}

impl MetricsCollector {
    pub fn new(monitored_paths: Vec<String>) -> Self {
        let (idle, total) = read_cpu_stats().unwrap_or((0, 0));
        let paths = if monitored_paths.is_empty() {
            vec!["/".to_string()]
        } else {
            monitored_paths
        };
        Self {
            prev_idle: idle,
            prev_total: total,
            monitored_paths: paths,
        }
    }

    pub fn collect(&mut self) -> HostMetrics {
        let (idle, total) = read_cpu_stats().unwrap_or((0, 0));
        let cpu_pct = if total > self.prev_total {
            let total_diff = total - self.prev_total;
            let idle_diff = idle.saturating_sub(self.prev_idle);
            100.0 * (1.0 - (idle_diff as f64 / total_diff as f64))
        } else {
            0.0
        };
        self.prev_idle = idle;
        self.prev_total = total;

        let (ram_pct, ram_used, ram_total) = read_mem_stats().unwrap_or((0.0, 0, 0));
        let uptime = read_uptime().unwrap_or(0);

        let mut worst_disk_pct = 0.0;
        let mut worst_disk_path = "/".to_string();

        for path in &self.monitored_paths {
            if let Ok(pct) = read_disk_usage(path) {
                if pct > worst_disk_pct {
                    worst_disk_pct = pct;
                    worst_disk_path = path.clone();
                }
            }
        }

        HostMetrics {
            os_name: detect_os(),
            uptime_secs: uptime,
            cpu_percent: (cpu_pct * 10.0).round() / 10.0,
            ram_percent: (ram_pct * 10.0).round() / 10.0,
            ram_used_mb: ram_used,
            ram_total_mb: ram_total,
            disk_percent: (worst_disk_pct * 10.0).round() / 10.0,
            disk_worst_path: worst_disk_path,
        }
    }
}

fn read_cpu_stats() -> Option<(u64, u64)> {
    let stat = fs::read_to_string("/proc/stat").ok()?;
    let first_line = stat.lines().next()?;
    if !first_line.starts_with("cpu ") {
        return None;
    }
    let parts: Vec<u64> = first_line
        .split_whitespace()
        .skip(1)
        .filter_map(|s| s.parse().ok())
        .collect();

    if parts.len() < 4 {
        return None;
    }

    let idle = parts[3] + parts.get(4).unwrap_or(&0);
    let total: u64 = parts.iter().sum();
    Some((idle, total))
}

fn read_mem_stats() -> Option<(f64, u64, u64)> {
    let meminfo = fs::read_to_string("/proc/meminfo").ok()?;
    let mut total_kb: u64 = 0;
    let mut available_kb: u64 = 0;

    for line in meminfo.lines() {
        if line.starts_with("MemTotal:") {
            total_kb = parse_meminfo_line(line);
        } else if line.starts_with("MemAvailable:") {
            available_kb = parse_meminfo_line(line);
        }
    }

    if total_kb == 0 {
        return None;
    }

    let used_kb = total_kb.saturating_sub(available_kb);
    let pct = (used_kb as f64 / total_kb as f64) * 100.0;
    Some((pct, used_kb / 1024, total_kb / 1024))
}

fn parse_meminfo_line(line: &str) -> u64 {
    line.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

fn read_uptime() -> Option<u64> {
    let content = fs::read_to_string("/proc/uptime").ok()?;
    content
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .map(|f| f as u64)
}

fn read_disk_usage(path: &str) -> Result<f64, nix::Error> {
    let stat = nix::sys::statvfs::statvfs(path)?;
    let total = stat.blocks() * stat.fragment_size();
    let free = stat.blocks_available() * stat.fragment_size();
    if total == 0 {
        return Ok(0.0);
    }
    let used = total.saturating_sub(free);
    Ok((used as f64 / total as f64) * 100.0)
}

fn detect_os() -> String {
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("PRETTY_NAME=") {
                return line.trim_start_matches("PRETTY_NAME=").trim_matches('"').to_string();
            }
        }
    }
    "Linux".to_string()
}
