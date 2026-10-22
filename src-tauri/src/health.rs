use crate::model::{now_unix_ms, HealthState, ItemHealth};
use std::io::ErrorKind;
use std::time::Duration;

pub fn next_health(
    prev: &ItemHealth,
    ok: bool,
    latency_ms: Option<u32>,
    reason: Option<String>,
) -> ItemHealth {
    let now = now_unix_ms();
    if ok {
        return ItemHealth {
            state: HealthState::Up,
            last_ok_ts: Some(now),
            last_checked_ts: Some(now),
            changed_ts: if prev.state == HealthState::Up {
                prev.changed_ts
            } else {
                Some(now)
            },
            latency_ms,
            reason: None,
        };
    }
    ItemHealth {
        state: HealthState::Degraded,
        last_ok_ts: prev.last_ok_ts,
        last_checked_ts: Some(now),
        changed_ts: if prev.state == HealthState::Degraded {
            prev.changed_ts
        } else {
            Some(now)
        },
        latency_ms: None,
        reason,
    }
}

pub fn apply_probe(
    prev: &ItemHealth,
    fail_streak: u32,
    ok: bool,
    latency_ms: Option<u32>,
    reason: Option<String>,
) -> (ItemHealth, u32) {
    if ok {
        return (next_health(prev, true, latency_ms, None), 0);
    }
    let streak = fail_streak.saturating_add(1);
    if streak >= 2 {
        let now = now_unix_ms();
        (
            ItemHealth {
                state: HealthState::Down,
                last_ok_ts: prev.last_ok_ts,
                last_checked_ts: Some(now),
                changed_ts: if prev.state == HealthState::Down {
                    prev.changed_ts
                } else {
                    Some(now)
                },
                latency_ms: None,
                reason,
            },
            streak,
        )
    } else {
        (next_health(prev, false, None, reason), streak)
    }
}

pub async fn tcp_alive(
    host: &str,
    port: u16,
    timeout: Duration,
) -> (bool, Option<u32>, Option<String>) {
    let start = std::time::Instant::now();
    match tokio::time::timeout(timeout, tokio::net::TcpStream::connect((host, port))).await {
        Ok(Ok(_stream)) => {
            let ms = start.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
            (true, Some(ms), None)
        }
        Err(_) => (false, None, Some("连接超时".into())),
        Ok(Err(err)) => {
            let reason = match err.kind() {
                ErrorKind::ConnectionRefused => "连接被拒绝".into(),
                ErrorKind::TimedOut => "连接超时".into(),
                ErrorKind::NotFound | ErrorKind::AddrNotAvailable => "地址无法解析".into(),
                ErrorKind::NetworkUnreachable => "网络不可达".into(),
                _ => format!("无法连接：{err}"),
            };
            (false, None, Some(reason))
        }
    }
}

pub fn unknown() -> ItemHealth {
    ItemHealth {
        state: HealthState::Unknown,
        last_ok_ts: None,
        last_checked_ts: None,
        changed_ts: None,
        latency_ms: None,
        reason: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn two_failures_turn_red() {
        let mut health = unknown();
        let mut streak = 0;
        let (h1, s1) = apply_probe(&health, streak, false, None, Some("连接超时".into()));
        health = h1;
        streak = s1;
        assert_eq!(health.state, HealthState::Degraded);
        assert_eq!(health.reason.as_deref(), Some("连接超时"));
        let (h2, _) = apply_probe(&health, streak, false, None, Some("连接超时".into()));
        assert_eq!(h2.state, HealthState::Down);
    }

    #[test]
    fn success_is_immediate_green() {
        let (health, streak) = apply_probe(&unknown(), 1, true, Some(12), None);
        assert_eq!(health.state, HealthState::Up);
        assert_eq!(streak, 0);
        assert_eq!(health.latency_ms, Some(12));
    }

    #[tokio::test]
    async fn live_local_listener_is_up_and_closed_port_is_down() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (up, ms, reason) = tcp_alive("127.0.0.1", port, Duration::from_secs(1)).await;
        assert!(up);
        assert!(ms.is_some());
        assert!(reason.is_none());
        drop(listener);
        let (down, _, reason) = tcp_alive("127.0.0.1", port, Duration::from_millis(400)).await;
        assert!(!down);
        assert!(reason.is_some());
    }
}
