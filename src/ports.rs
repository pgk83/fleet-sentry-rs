use std::fs;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct OpenPort {
    pub port: u16,
    pub is_wildcard: bool,
    pub protocol: String,
    pub service_hint: String,
}

pub fn scan_listening_ports() -> Vec<OpenPort> {
    let mut ports = Vec::new();

    if let Ok(content) = fs::read_to_string("/proc/net/tcp") {
        parse_proc_net_tcp(&content, "tcp4", &mut ports);
    }

    if let Ok(content) = fs::read_to_string("/proc/net/tcp6") {
        parse_proc_net_tcp(&content, "tcp6", &mut ports);
    }

    ports.sort_by_key(|p| p.port);
    ports.dedup_by_key(|p| (p.port, p.is_wildcard));
    ports
}

fn parse_proc_net_tcp(content: &str, proto: &str, out: &mut Vec<OpenPort>) {
    for line in content.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }

        // State "0A" indicates TCP_LISTEN
        if parts[3] != "0A" {
            continue;
        }

        // Local address is formatted as IP_HEX:PORT_HEX
        let addr_parts: Vec<&str> = parts[1].split(':').collect();
        if addr_parts.len() != 2 {
            continue;
        }

        let ip_hex = addr_parts[0];
        let port = match u16::from_str_radix(addr_parts[1], 16) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let is_wildcard = ip_hex.chars().all(|c| c == '0');
        let hint = port_hint(port);

        out.push(OpenPort {
            port,
            is_wildcard,
            protocol: proto.to_string(),
            service_hint: hint,
        });
    }
}

fn port_hint(port: u16) -> String {
    match port {
        22 => "SSH",
        80 => "HTTP",
        443 => "HTTPS",
        3000 => "Gitea / Grafana",
        4000 => "LiteLLM",
        4533 => "Navidrome",
        5432 => "PostgreSQL",
        6379 => "Redis",
        7878 => "Radarr",
        8080 => "qBittorrent WebUI",
        8989 => "Sonarr",
        9090 => "Prometheus / qBittorrent",
        9099 => "Watchdog Control RPC",
        9117 => "Jackett",
        11434 => "Ollama",
        32400 => "Plex Media Server",
        _ => "Unknown",
    }.to_string()
}
