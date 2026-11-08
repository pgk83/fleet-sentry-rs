mod metrics;
mod ports;
mod docker;

use std::env;
use std::fs;
use std::time::Duration;
use tokio::time::sleep;
use log::{info, warn};
use metrics::MetricsCollector;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let node_name = env::var("NODE_NAME")
        .or_else(|_| fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string()))
        .unwrap_or_else(|_| "node-unknown".to_string());

    let interval_secs: u64 = env::var("SCAN_INTERVAL")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);

    let run_once = env::args().any(|a| a == "--once");

    let docker_socket = env::var("DOCKER_SOCKET")
        .unwrap_or_else(|_| "/var/run/docker.sock".to_string());

    info!("🚀 Fleet Sentry (Rust static micro-daemon) starting on node '{}' (interval: {}s)", node_name, interval_secs);

    let mut collector = MetricsCollector::new(vec!["/".to_string(), "/mnt/user".to_string(), "/media/sda1".to_string()]);

    loop {
        let host_m = collector.collect();
        let open_ports = ports::scan_listening_ports();
        let wildcard_ports: Vec<_> = open_ports.iter().filter(|p| p.is_wildcard).collect();

        let containers = match docker::list_containers(&docker_socket).await {
            Ok(c) => c,
            Err(e) => {
                warn!("Docker scan warning: {}", e);
                Vec::new()
            }
        };

        let running_containers = containers.iter().filter(|c| c.is_running).count();

        info!(
            "[{}] CPU: {:>4.1}% | RAM: {:>4.1}% ({}/{} MB) | Disk ({}): {:>4.1}% | Ports: {} (wildcard: {}) | Containers: {}/{}",
            node_name,
            host_m.cpu_percent,
            host_m.ram_percent,
            host_m.ram_used_mb,
            host_m.ram_total_mb,
            host_m.disk_worst_path,
            host_m.disk_percent,
            open_ports.len(),
            wildcard_ports.len(),
            running_containers,
            containers.len()
        );

        // Alert on critical resource thresholds
        if host_m.cpu_percent > 95.0 {
            warn!("⚠️ HIGH CPU: {:.1}% on {}", host_m.cpu_percent, node_name);
        }
        if host_m.ram_percent > 95.0 {
            warn!("⚠️ HIGH RAM: {:.1}% on {}", host_m.ram_percent, node_name);
        }
        if host_m.disk_percent > 92.0 {
            warn!("⚠️ HIGH DISK: {:.1}% on {} ({})", host_m.disk_percent, node_name, host_m.disk_worst_path);
        }

        if run_once {
            return;
        }

        sleep(Duration::from_secs(interval_secs)).await;
    }
}
