use std::path::Path;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub state: String,
    pub status: String,
    pub is_running: bool,
}

#[derive(Deserialize)]
struct RawContainer {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Names")]
    names: Option<Vec<String>>,
    #[serde(rename = "State")]
    state: String,
    #[serde(rename = "Status")]
    status: String,
}

pub async fn list_containers(socket_path: &str) -> Result<Vec<ContainerInfo>, Box<dyn std::error::Error + Send + Sync>> {
    if !Path::new(socket_path).exists() {
        return Ok(Vec::new());
    }

    let mut stream = match UnixStream::connect(socket_path).await {
        Ok(s) => s,
        Err(e) => return Err(Box::new(e)),
    };

    let request = "GET /containers/json?all=1 HTTP/1.0\r\nHost: docker\r\n\r\n";
    stream.write_all(request.as_bytes()).await?;

    let mut response_bytes = Vec::new();
    stream.read_to_end(&mut response_bytes).await?;

    let response_str = String::from_utf8_lossy(&response_bytes);

    // Split headers and body
    let body = match response_str.split("\r\n\r\n").nth(1) {
        Some(b) => b,
        None => return Ok(Vec::new()),
    };

    let raw: Vec<RawContainer> = serde_json::from_str(body)?;

    let containers = raw
        .into_iter()
        .map(|c| {
            let name = c
                .names
                .and_then(|ns| ns.first().cloned())
                .map(|n| n.trim_start_matches('/').to_string())
                .unwrap_or_else(|| c.id.chars().take(12).collect());
            let is_running = c.state.eq_ignore_ascii_case("running");
            ContainerInfo {
                id: c.id.chars().take(12).collect(),
                name,
                state: c.state,
                status: c.status,
                is_running,
            }
        })
        .collect();

    Ok(containers)
}
