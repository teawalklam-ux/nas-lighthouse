use crate::config::TitleRules;
use crate::model::{ActiveAccess, ContainerStatus, NavItem};
use crate::nav::{container_for_port, parse_host_ip};
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct RemoteConn {
    pub ip: IpAddr,
    pub port: u16,
}

/// 浏览器常在读完页面后关掉 TCP，悬浮窗轮询会漏。短时记住刚见过的端口。
pub struct AccessDwell {
    last: HashMap<u16, (IpAddr, Instant)>,
    hold: Duration,
}

impl Default for AccessDwell {
    fn default() -> Self {
        Self {
            last: HashMap::new(),
            hold: Duration::from_secs(180),
        }
    }
}

impl AccessDwell {
    #[cfg(test)]
    fn with_hold(hold: Duration) -> Self {
        Self {
            last: HashMap::new(),
            hold,
        }
    }

    pub fn observe(&mut self, conns: Vec<RemoteConn>) -> Vec<RemoteConn> {
        let now = Instant::now();
        for conn in &conns {
            self.last.insert(conn.port, (conn.ip, now));
        }
        self.last
            .retain(|_, (_, seen)| now.saturating_duration_since(*seen) < self.hold);
        let have: HashSet<u16> = conns.iter().map(|conn| conn.port).collect();
        let mut out = conns;
        for (port, (ip, _)) in &self.last {
            if !have.contains(port) {
                out.push(RemoteConn {
                    ip: *ip,
                    port: *port,
                });
            }
        }
        out
    }
}

fn canon_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6
            .to_ipv4_mapped()
            .map(IpAddr::V4)
            .unwrap_or(IpAddr::V6(v6)),
        other => other,
    }
}

fn lan_like(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local() || v4.is_loopback(),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return v4.is_private() || v4.is_link_local() || v4.is_loopback();
            }
            let segs = v6.segments();
            v6.is_loopback() || (segs[0] & 0xfe00) == 0xfc00 || (segs[0] & 0xffc0) == 0xfe80
        }
    }
}

/// Ports worth attributing to the NAS: whatever the user recorded, whatever
/// Docker publishes, plus any `watchPorts` they added by hand.
fn watched_ports(items: &[NavItem], containers: &[ContainerStatus], extra: &[u16]) -> HashSet<u16> {
    items
        .iter()
        .map(|item| item.port)
        .chain(
            containers
                .iter()
                .flat_map(|c| c.ports.iter().map(|p| p.host)),
        )
        .chain(extra.iter().copied())
        .collect()
}

fn counts_as_nas_access(ip: IpAddr, port: u16, nas: &[IpAddr], extra_ports: &HashSet<u16>) -> bool {
    let ip = canon_ip(ip);
    nas.contains(&ip) || (extra_ports.contains(&port) && lan_like(ip))
}

fn live_tcp(state: netstat2::TcpState) -> bool {
    use netstat2::TcpState::*;
    matches!(
        state,
        Established | SynSent | SynReceived | FinWait1 | FinWait2 | CloseWait | TimeWait
    )
}

pub fn match_access(
    conns: &[RemoteConn],
    nas_ips: &[IpAddr],
    items: &[NavItem],
    containers: &[ContainerStatus],
    nas_host: &str,
    watch_ports: &[u16],
    rules: &TitleRules,
) -> Vec<ActiveAccess> {
    let nas: Vec<IpAddr> = nas_ips.iter().copied().map(canon_ip).collect();
    let extra_ports = watched_ports(items, containers, watch_ports);
    let mut counts: HashMap<u16, u32> = HashMap::new();
    for conn in conns {
        if counts_as_nas_access(conn.ip, conn.port, &nas, &extra_ports) {
            *counts.entry(conn.port).or_insert(0) += 1;
        }
    }

    let mut out = Vec::new();
    for (port, connections) in counts {
        if crate::nav::skipped_port(port) {
            continue;
        }
        let item = items.iter().find(|item| {
            item.port == port
                && (ip_of(&item.host).is_some_and(|ip| nas.contains(&canon_ip(ip)))
                    || (!nas_host.is_empty() && item.host.eq_ignore_ascii_case(nas_host)))
        });
        let container = container_for_port(containers, port);
        out.push(ActiveAccess {
            port,
            nav_item_id: item.map(|i| i.id.clone()),
            title: item.map(|i| i.title.clone()).or_else(|| {
                Some(crate::nav::suggest_title(
                    container.map(|c| c.name.as_str()),
                    port,
                    rules,
                ))
            }),
            container_name: item
                .and_then(|i| i.container_name.clone())
                .or_else(|| container.map(|c| c.name.clone())),
            connections,
        });
    }
    out.sort_by_key(|a| a.port);
    out
}

fn ip_of(host: &str) -> Option<IpAddr> {
    parse_host_ip(host)
}

pub fn collect_established(nas_ips: &[IpAddr], extra_ports: &[u16]) -> Vec<RemoteConn> {
    use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo};

    let nas: Vec<IpAddr> = nas_ips.iter().copied().map(canon_ip).collect();
    let extra = watched_ports(&[], &[], extra_ports);
    let mut out = Vec::new();
    // IPv4 / IPv6 分开取：一张表失败时另一张还能用。
    for family in [AddressFamilyFlags::IPV4, AddressFamilyFlags::IPV6] {
        let Ok(sockets) = get_sockets_info(family, ProtocolFlags::TCP) else {
            continue;
        };
        for info in sockets {
            let ProtocolSocketInfo::Tcp(tcp) = info.protocol_socket_info else {
                continue;
            };
            if !live_tcp(tcp.state) {
                continue;
            }
            if counts_as_nas_access(tcp.remote_addr, tcp.remote_port, &nas, &extra) {
                out.push(RemoteConn {
                    ip: tcp.remote_addr,
                    port: tcp.remote_port,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NavItem;
    use crate::nav::http_url;
    use std::net::Ipv4Addr;

    const HOST: &str = "192.0.2.10";

    fn nas_ip() -> IpAddr {
        Ipv4Addr::new(192, 0, 2, 10).into()
    }

    fn item(port: u16, title: &str) -> NavItem {
        NavItem {
            id: format!("n{port}"),
            title: title.into(),
            icon: "box".into(),
            url: http_url(HOST, port),
            host: HOST.into(),
            port,
            container_name: Some("media-jellyfin-1".into()),
            group: "NAS 服务".into(),
            order: 0,
            favorite: false,
            health_check: true,
        }
    }

    fn access_of(
        conns: &[RemoteConn],
        items: &[NavItem],
        containers: &[ContainerStatus],
        host: &str,
    ) -> Vec<ActiveAccess> {
        match_access(
            conns,
            &[nas_ip()],
            items,
            containers,
            host,
            &[],
            &TitleRules::new(),
        )
    }

    #[test]
    fn matches_recorded_and_keeps_unknown_host_ports() {
        let nas = nas_ip();
        let conns = vec![
            RemoteConn {
                ip: nas,
                port: 8096,
            },
            RemoteConn {
                ip: nas,
                port: 8096,
            },
            RemoteConn {
                ip: nas,
                port: 9443,
            },
            RemoteConn {
                ip: Ipv4Addr::new(1, 1, 1, 1).into(),
                port: 443,
            },
        ];
        let access = access_of(&conns, &[item(8096, "Jellyfin")], &[], HOST);
        assert_eq!(access.len(), 2);
        let recorded = access.iter().find(|a| a.port == 8096).unwrap();
        assert_eq!(recorded.nav_item_id.as_deref(), Some("n8096"));
        assert_eq!(recorded.title.as_deref(), Some("Jellyfin"));
        assert_eq!(recorded.connections, 2);
        let admin = access.iter().find(|a| a.port == 9443).unwrap();
        assert!(admin.nav_item_id.is_none());
        assert_eq!(admin.title.as_deref(), Some("NAS 管理界面"));
    }

    #[test]
    fn skips_ssh_and_other_system_ports() {
        let nas = nas_ip();
        let conns = vec![
            RemoteConn { ip: nas, port: 22 },
            RemoteConn { ip: nas, port: 445 },
            RemoteConn {
                ip: nas,
                port: 22000,
            },
            RemoteConn {
                ip: nas,
                port: 8096,
            },
        ];
        let access = access_of(&conns, &[item(8096, "Jellyfin")], &[], HOST);
        assert_eq!(access.len(), 1);
        assert_eq!(access[0].port, 8096);
    }

    #[test]
    fn matches_docker_port_on_ula_ipv6() {
        use crate::model::PortMap;
        use std::net::Ipv6Addr;
        let ula: IpAddr = Ipv6Addr::new(0xfd12, 0, 0, 0, 0, 0, 0, 1).into();
        let containers = vec![ContainerStatus {
            name: "immich-server-1".into(),
            image: "x".into(),
            state: "running".into(),
            status: "Up".into(),
            health: None,
            ports: vec![PortMap {
                host: 2283,
                container: 3001,
                proto: "tcp".into(),
            }],
        }];
        let conns = vec![RemoteConn {
            ip: ula,
            port: 2283,
        }];
        let access = access_of(&conns, &[], &containers, HOST);
        assert_eq!(access.len(), 1);
        assert_eq!(access[0].port, 2283);
        assert_eq!(access[0].title.as_deref(), Some("Immich"));
    }

    #[test]
    fn hostname_nav_item_still_matches_access() {
        let nas = nas_ip();
        let mut rec = item(8096, "Jellyfin");
        rec.host = "NAS.local".into();
        let conns = vec![RemoteConn {
            ip: nas,
            port: 8096,
        }];
        let access = access_of(&conns, &[rec], &[], "nas.local");
        assert_eq!(access[0].nav_item_id.as_deref(), Some("n8096"));
    }

    #[test]
    fn configured_watch_ports_match_without_nav_or_docker() {
        use std::net::Ipv6Addr;
        let ula: IpAddr = Ipv6Addr::new(0xfd12, 0, 0, 0, 0, 0, 0, 1).into();
        let conns = vec![
            RemoteConn {
                ip: ula,
                port: 8080,
            },
            RemoteConn {
                ip: ula,
                port: 8443,
            },
        ];
        // Nothing is watched by default: a LAN address that is not the NAS only
        // counts once the user opts the port in.
        let bare = access_of(&conns, &[], &[], HOST);
        assert!(bare.is_empty());
        let opted = match_access(
            &conns,
            &[nas_ip()],
            &[],
            &[],
            HOST,
            &[8080, 8443],
            &TitleRules::new(),
        );
        let ports: Vec<u16> = opted.iter().map(|a| a.port).collect();
        assert_eq!(ports, vec![8080, 8443]);
    }

    #[test]
    fn dwell_keeps_ports_after_socket_closes() {
        let nas = nas_ip();
        let mut dwell = AccessDwell::with_hold(Duration::from_secs(60));
        let first = dwell.observe(vec![RemoteConn {
            ip: nas,
            port: 8096,
        }]);
        assert_eq!(first.len(), 1);
        let held = dwell.observe(Vec::new());
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].port, 8096);
    }
}
