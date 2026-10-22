use crate::model::{ContainerStatus, PortMap};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct DockerPsRow {
    #[serde(default, rename = "Names")]
    names: String,
    #[serde(default, rename = "Image")]
    image: String,
    #[serde(default, rename = "State")]
    state: String,
    #[serde(default, rename = "Status")]
    status: String,
    #[serde(default, rename = "HealthStatus")]
    health_status: String,
    #[serde(default, rename = "Ports")]
    ports: String,
}

pub fn parse_ports(ports: &str) -> Vec<PortMap> {
    let mut out = Vec::new();
    for part in ports.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some(arrow) = part.find("->") else {
            continue;
        };
        let left = part[..arrow].trim();
        let right = part[arrow + 2..].trim();
        let host_str = left.rsplit(':').next().unwrap_or("");
        let mut right_parts = right.split('/');
        let container_str = right_parts.next().unwrap_or("");
        let proto = right_parts.next().unwrap_or("tcp");
        let Ok(host) = host_str.parse::<u16>() else {
            continue;
        };
        let Ok(container) = container_str.parse::<u16>() else {
            continue;
        };
        let mapped = PortMap {
            host,
            container,
            proto: proto.to_string(),
        };
        if !out.iter().any(|p| p == &mapped) {
            out.push(mapped);
        }
    }
    out
}

pub fn parse_docker_line(json_line: &str) -> Option<ContainerStatus> {
    let row: DockerPsRow = serde_json::from_str(json_line).ok()?;
    if row.names.is_empty() {
        return None;
    }
    let health = match row.health_status.as_str() {
        "" | "none" => None,
        other => Some(other.to_string()),
    };
    Some(ContainerStatus {
        name: row.names,
        image: row.image,
        state: row.state,
        status: row.status,
        health,
        ports: parse_ports(&row.ports),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_and_ipv6_same_port_dedupes() {
        let ports = parse_ports("0.0.0.0:8096->8096/tcp, [::]:8096->8096/tcp");
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].host, 8096);
        assert_eq!(ports[0].container, 8096);
        assert_eq!(ports[0].proto, "tcp");
    }

    #[test]
    fn alist_two_host_ports() {
        let ports = parse_ports(
            "0.0.0.0:15244->5244/tcp, [::]:15244->5244/tcp, 0.0.0.0:15245->5245/tcp, [::]:15245->5245/tcp",
        );
        assert_eq!(ports.len(), 2);
        assert_eq!(ports[0].host, 15244);
        assert_eq!(ports[0].container, 5244);
        assert_eq!(ports[1].host, 15245);
        assert_eq!(ports[1].container, 5245);
    }

    #[test]
    fn empty_and_unmapped_are_skipped() {
        assert!(parse_ports("").is_empty());
        assert!(parse_ports("8096/tcp").is_empty());
    }

    #[test]
    fn docker_json_line_from_real_shape() {
        let line = r#"{"Names":"media-jellyfin-media-jellyfin-1","Image":"jellyfin/jellyfin","State":"running","Status":"Up 2 hours (healthy)","HealthStatus":"healthy","Ports":"0.0.0.0:8096->8096/tcp, [::]:8096->8096/tcp"}"#;
        let c = parse_docker_line(line).unwrap();
        assert_eq!(c.name, "media-jellyfin-media-jellyfin-1");
        assert_eq!(c.health.as_deref(), Some("healthy"));
        assert_eq!(c.ports[0].host, 8096);
    }
}
