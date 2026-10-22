use crate::config::TitleRules;
use crate::model::{now_unix_ms, Candidate, CandidateSource, ContainerStatus, NavItem};
use std::collections::HashSet;
use std::net::IpAddr;

/// Infrastructure ports that are never a web UI: SSH, DNS, SMB, NFS, AFP,
/// mDNS/LLMNR, Windows delivery optimisation, Syncthing sync.
const SKIP_PORTS: &[u16] = &[22, 53, 139, 445, 548, 2049, 5353, 5355, 7680, 22000];

/// Ports NAS vendors and self-hosted stacks conventionally serve over TLS.
const HTTPS_PORTS: &[u16] = &[5001, 9443, 9999];

pub fn skipped_port(port: u16) -> bool {
    SKIP_PORTS.contains(&port)
}

pub fn http_url(host: &str, port: u16) -> String {
    let host = host_for_url(host);
    match port {
        80 => format!("http://{host}/"),
        443 => format!("https://{host}/"),
        p if HTTPS_PORTS.contains(&p) => format!("https://{host}:{port}/"),
        _ => format!("http://{host}:{port}/"),
    }
}

fn host_for_url(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

/// 从 http(s) URL 取出探测用的 host/port。IPv6 存成不带方括号的形式。
pub fn host_port_from_http_url(url: &str) -> Option<(String, u16)> {
    let url = url.trim();
    let (https, rest) = if let Some(rest) = url.strip_prefix("https://") {
        (true, rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        (false, rest)
    } else {
        return None;
    };
    if rest.is_empty() || url.chars().any(char::is_whitespace) {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return None;
    }
    let hostport = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);
    if hostport.is_empty() {
        return None;
    }
    if let Some(rest) = hostport.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        if host.is_empty() {
            return None;
        }
        let port = if after.is_empty() {
            if https {
                443
            } else {
                80
            }
        } else {
            after.strip_prefix(':')?.parse().ok()?
        };
        return (port != 0).then_some((host.to_string(), port));
    }
    match hostport.rsplit_once(':') {
        Some((host, port))
            if !host.is_empty()
                && !host.contains(':')
                && port.chars().all(|c| c.is_ascii_digit()) =>
        {
            let port: u16 = port.parse().ok()?;
            (port != 0).then_some((host.to_string(), port))
        }
        _ if !hostport.contains(':') => Some((hostport.to_string(), if https { 443 } else { 80 })),
        _ => None,
    }
}

pub fn valid_http_url(url: &str) -> bool {
    host_port_from_http_url(url).is_some()
}

pub fn apply_http_endpoint(item: &mut NavItem) -> Result<(), String> {
    let (host, port) = host_port_from_http_url(&item.url)
        .ok_or_else(|| format!("无法从地址解析主机和端口：{}", item.title))?;
    item.url = item.url.trim().to_string();
    item.host = host;
    item.port = port;
    Ok(())
}

/// Display names for widely deployed self-hosted projects. Only upstream
/// open-source software belongs here — anything specific to one person's NAS
/// goes in their own `containerTitles`, not in the repo.
const KNOWN_CONTAINERS: &[(&str, &str)] = &[
    ("adguard", "AdGuard Home"),
    ("alist", "AList"),
    ("audiobookshelf", "Audiobookshelf"),
    ("calibre", "Calibre"),
    ("emby", "Emby"),
    ("filebrowser", "File Browser"),
    ("firefox", "Firefox"),
    ("gitea", "Gitea"),
    ("grafana", "Grafana"),
    ("homeassistant", "Home Assistant"),
    ("immich", "Immich"),
    ("jellyfin", "Jellyfin"),
    ("n8n", "n8n"),
    ("navidrome", "Navidrome"),
    ("nextcloud", "Nextcloud"),
    ("nginx", "nginx"),
    ("nocodb", "NocoDB"),
    ("open-webui", "Open WebUI"),
    ("photoprism", "PhotoPrism"),
    ("portainer", "Portainer"),
    ("prometheus", "Prometheus"),
    ("qbittorrent", "qBittorrent"),
    ("radarr", "Radarr"),
    ("sillytavern", "SillyTavern"),
    ("siyuan", "SiYuan"),
    ("sonarr", "Sonarr"),
    ("tailscale", "Tailscale"),
    ("transmission", "Transmission"),
    ("uptime-kuma", "Uptime Kuma"),
    ("vaultwarden", "Vaultwarden"),
    ("wallos", "Wallos"),
];

/// Ports a NAS conventionally serves its own admin console on. Worded without
/// naming a vendor: 9443 is UGOS and Portainer, 5000/5001 is DSM.
fn admin_port_title(port: u16) -> Option<&'static str> {
    match port {
        80 => Some("NAS HTTP"),
        443 => Some("NAS HTTPS"),
        5000 | 5001 | 9443 | 9999 => Some("NAS 管理界面"),
        _ => None,
    }
}

pub fn suggest_title(container_name: Option<&str>, port: u16, rules: &TitleRules) -> String {
    if let Some(name) = container_name {
        let title = container_title(name, rules);
        if title != name {
            return title;
        }
        // Unrecognised container on an admin port: the port says more than the
        // compose-generated name does.
        return admin_port_title(port).map_or(title, str::to_string);
    }
    if let Some(admin) = admin_port_title(port) {
        return admin.to_string();
    }
    format!("端口 {port}")
}

/// User rules win over the built-in list, so a private service can be named
/// without patching the binary. Unknown containers keep their raw name.
pub fn container_title(name: &str, rules: &TitleRules) -> String {
    let lower = name.to_lowercase();
    for (needle, title) in rules {
        let needle = needle.trim().to_lowercase();
        if !needle.is_empty() && lower.contains(&needle) {
            return title.clone();
        }
    }
    for (needle, title) in KNOWN_CONTAINERS {
        if lower.contains(needle) {
            return (*title).to_string();
        }
    }
    name.to_string()
}

fn recorded_keys(items: &[NavItem]) -> HashSet<(String, u16)> {
    items.iter().map(|i| (i.host.clone(), i.port)).collect()
}

pub fn container_for_port(containers: &[ContainerStatus], port: u16) -> Option<&ContainerStatus> {
    containers
        .iter()
        .find(|c| c.ports.iter().any(|p| p.host == port))
}

pub fn candidates(
    nas_host: &str,
    items: &[NavItem],
    containers: &[ContainerStatus],
    unmatched_ports: &[(u16, u32)],
    rules: &TitleRules,
) -> Vec<Candidate> {
    // No host means no URL to suggest; the UI asks for setup instead.
    if nas_host.trim().is_empty() {
        return Vec::new();
    }
    let recorded = recorded_keys(items);
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    for container in containers {
        for port in &container.ports {
            if port.proto != "tcp" || SKIP_PORTS.contains(&port.host) {
                continue;
            }
            let key = (nas_host.to_string(), port.host);
            if recorded.contains(&key) || !seen.insert(key.clone()) {
                continue;
            }
            out.push(Candidate {
                host: nas_host.to_string(),
                port: port.host,
                suggested_title: suggest_title(Some(&container.name), port.host, rules),
                url: http_url(nas_host, port.host),
                container_name: Some(container.name.clone()),
                source: CandidateSource::Docker,
                proto: port.proto.clone(),
            });
        }
    }

    for (port, _) in unmatched_ports {
        if SKIP_PORTS.contains(port) {
            continue;
        }
        let key = (nas_host.to_string(), *port);
        if recorded.contains(&key) || !seen.insert(key) {
            continue;
        }
        let container = container_for_port(containers, *port);
        out.push(Candidate {
            host: nas_host.to_string(),
            port: *port,
            suggested_title: suggest_title(container.map(|c| c.name.as_str()), *port, rules),
            url: http_url(nas_host, *port),
            container_name: container.map(|c| c.name.clone()),
            source: CandidateSource::Access,
            proto: "tcp".to_string(),
        });
    }

    out.sort_by_key(|c| c.port);
    out
}

pub fn item_from_candidate(candidate: &Candidate, order: u32) -> Result<NavItem, String> {
    let mut item = NavItem {
        id: format!("n{}", now_unix_ms()),
        title: candidate.suggested_title.clone(),
        icon: default_icon_for(candidate),
        url: candidate.url.clone(),
        host: candidate.host.clone(),
        port: candidate.port,
        container_name: candidate.container_name.clone(),
        group: "NAS 服务".to_string(),
        order,
        favorite: false,
        health_check: true,
    };
    apply_http_endpoint(&mut item)?;
    Ok(item)
}

const VIDEO_HINTS: &[&str] = &["jellyfin", "emby", "plex", "video", "media"];

fn default_icon_for(candidate: &Candidate) -> String {
    if admin_port_title(candidate.port).is_some() {
        return "monitor".to_string();
    }
    let name = candidate
        .container_name
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();
    if VIDEO_HINTS.iter().any(|hint| name.contains(hint)) {
        return "video".to_string();
    }
    "box".to_string()
}

pub fn parse_host_ip(host: &str) -> Option<IpAddr> {
    host.parse().ok()
}

pub async fn resolve_host_ips(host: &str) -> Vec<IpAddr> {
    let host = host.trim();
    if host.is_empty() {
        return Vec::new();
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return vec![ip];
    }
    match tokio::net::lookup_host((host, 0)).await {
        Ok(addrs) => {
            let mut out = Vec::new();
            for addr in addrs {
                let ip = addr.ip();
                if !out.contains(&ip) {
                    out.push(ip);
                }
            }
            out
        }
        Err(_) => Vec::new(),
    }
}

/// Compose repeats the project name in container names:
/// `media-jellyfin-media-jellyfin-1` → `media-jellyfin-1`
#[cfg(test)]
pub fn compact_container_name(name: &str) -> String {
    let Some((body, replica)) = name.rsplit_once('-') else {
        return name.to_string();
    };
    if !replica.chars().all(|c| c.is_ascii_digit()) {
        return name.to_string();
    }
    if body.len() % 2 == 0 {
        return name.to_string();
    }
    let prefix_len = (body.len() - 1) / 2;
    if body.as_bytes().get(prefix_len) != Some(&b'-') {
        return name.to_string();
    }
    let left = &body[..prefix_len];
    let right = &body[prefix_len + 1..];
    if left == right && !left.is_empty() {
        return format!("{left}-{replica}");
    }
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ContainerStatus;
    use crate::parse::docker::parse_docker_line;

    const HOST: &str = "192.0.2.10";

    fn container(name: &str, ports: &str) -> ContainerStatus {
        let line = format!(
            r#"{{"Names":"{name}","Image":"x","State":"running","Status":"Up","HealthStatus":"none","Ports":"{ports}"}}"#
        );
        parse_docker_line(&line).unwrap()
    }

    #[test]
    fn https_for_admin_ports_and_http_for_the_rest() {
        assert_eq!(http_url(HOST, 9443), "https://192.0.2.10:9443/");
        assert_eq!(http_url(HOST, 5001), "https://192.0.2.10:5001/");
        assert_eq!(http_url(HOST, 8096), "http://192.0.2.10:8096/");
        assert_eq!(http_url(HOST, 80), "http://192.0.2.10/");
    }

    #[test]
    fn docker_candidates_skip_recorded_and_udp() {
        let media = container(
            "media-jellyfin-media-jellyfin-1",
            "0.0.0.0:8096->8096/tcp, [::]:8096->8096/tcp",
        );
        let tail = container("tailscale", "0.0.0.0:41641->41641/udp");
        let items = vec![NavItem {
            id: "x".into(),
            title: "Jellyfin".into(),
            icon: "box".into(),
            url: http_url(HOST, 8096),
            host: HOST.into(),
            port: 8096,
            container_name: Some(media.name.clone()),
            group: "NAS 服务".into(),
            order: 0,
            favorite: false,
            health_check: true,
        }];
        let found = candidates(HOST, &items, &[media, tail], &[], &TitleRules::new());
        assert!(found.iter().all(|c| c.port != 8096));
        assert!(found.iter().all(|c| c.port != 41641));
    }

    #[test]
    fn access_unmatched_becomes_host_candidate() {
        let found = candidates(HOST, &[], &[], &[(9443, 3)], &TitleRules::new());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].suggested_title, "NAS 管理界面");
        assert_eq!(found[0].source, CandidateSource::Access);
        assert!(found[0].container_name.is_none());
    }

    #[test]
    fn no_candidates_before_the_host_is_configured() {
        let found = candidates("", &[], &[], &[(9443, 3)], &TitleRules::new());
        assert!(found.is_empty());
    }

    #[test]
    fn rejects_javascript_url() {
        assert!(!valid_http_url("javascript:alert(1)"));
        assert!(valid_http_url("https://192.0.2.10:9443/"));
        assert!(!valid_http_url("http://"));
        assert!(!valid_http_url("https://example.com:99999/"));
        assert!(skipped_port(22000));
        assert!(!skipped_port(8096));
    }

    #[test]
    fn url_endpoint_overrides_stale_host_port() {
        let parsed = host_port_from_http_url("http://nas.local:8096/app").unwrap();
        assert_eq!(parsed, ("nas.local".into(), 8096));
        let v6 = host_port_from_http_url("http://[fd12::1]:8096/").unwrap();
        assert_eq!(v6, ("fd12::1".into(), 8096));
        assert_eq!(http_url("fd12::1", 8096), "http://[fd12::1]:8096/");
        let mut item = NavItem {
            id: "n1".into(),
            title: "Jellyfin".into(),
            icon: "video".into(),
            url: "https://192.0.2.10:9443/".into(),
            host: "198.51.100.1".into(),
            port: 80,
            container_name: None,
            group: "NAS 服务".into(),
            order: 1,
            favorite: false,
            health_check: true,
        };
        apply_http_endpoint(&mut item).unwrap();
        assert_eq!(item.host, "192.0.2.10");
        assert_eq!(item.port, 9443);
    }

    #[test]
    fn built_in_names_cover_common_projects() {
        let none = TitleRules::new();
        assert_eq!(container_title("media-jellyfin-1", &none), "Jellyfin");
        assert_eq!(container_title("stack-qbittorrent-1", &none), "qBittorrent");
        assert_eq!(
            container_title("whatever-custom-1", &none),
            "whatever-custom-1"
        );
        assert_eq!(suggest_title(None, 9443, &none), "NAS 管理界面");
        assert_eq!(suggest_title(None, 8096, &none), "端口 8096");
    }

    #[test]
    fn user_rules_win_over_built_in_and_raw_names() {
        let mut rules = TitleRules::new();
        rules.insert("jellyfin".into(), "影视库".into());
        rules.insert("my-notes".into(), "笔记".into());
        assert_eq!(container_title("media-jellyfin-1", &rules), "影视库");
        assert_eq!(container_title("my-notes-1", &rules), "笔记");
        assert_eq!(suggest_title(Some("my-notes-1"), 8080, &rules), "笔记");
        // Unrecognised container on an admin port falls back to the port's role.
        assert_eq!(
            suggest_title(Some("odd-name-1"), 9443, &rules),
            "NAS 管理界面"
        );
    }

    #[test]
    fn compact_repeated_compose_name() {
        assert_eq!(
            compact_container_name("media-jellyfin-media-jellyfin-1"),
            "media-jellyfin-1"
        );
        assert_eq!(
            compact_container_name("stack-app-stack-app-1"),
            "stack-app-1"
        );
        assert_eq!(compact_container_name("sillytavern"), "sillytavern");
        assert_eq!(compact_container_name("nocodb"), "nocodb");
        assert_eq!(compact_container_name("firefox-app-1"), "firefox-app-1");
    }

    #[tokio::test]
    async fn resolve_literal_ip_skips_dns() {
        assert_eq!(
            resolve_host_ips(HOST).await,
            vec![HOST.parse::<IpAddr>().unwrap()]
        );
        assert!(resolve_host_ips("").await.is_empty());
    }
}
