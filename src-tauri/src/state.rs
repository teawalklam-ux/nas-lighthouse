use crate::model::{
    now_unix_ms, CpuStatus, InternetState, InternetStatus, LinkState, MemStatus, MetricsSnapshot,
    NetStatus,
};
use crate::parse::block::RawSample;
use crate::parse::cpu::cpu_usage_pct;
use crate::parse::sys::net_rate_bytes_per_sec;
use crate::parse::temp::select_display_temps;

pub struct Reducer {
    prev_cpu: Option<crate::parse::cpu::CpuTimes>,
    prev_net: Option<(u64, u64, u128)>,
    last_containers: Vec<crate::model::ContainerStatus>,
    last_raids: Vec<crate::model::RaidStatus>,
    internet: InternetStatus,
    internet_fail_streak: u32,
    last_success_ts: Option<u64>,
    last_link_agent: Option<bool>,
}

impl Reducer {
    pub fn new() -> Self {
        Self {
            prev_cpu: None,
            prev_net: None,
            last_containers: Vec::new(),
            last_raids: Vec::new(),
            internet: InternetStatus {
                state: InternetState::Unknown,
                last_ok_ts: None,
                latency_ms: None,
                link_agent_alive: None,
            },
            internet_fail_streak: 0,
            last_success_ts: None,
            last_link_agent: None,
        }
    }

    pub fn reset_rates(&mut self) {
        self.prev_cpu = None;
        self.prev_net = None;
    }

    pub fn apply(&mut self, sample: RawSample, link: LinkState) -> MetricsSnapshot {
        let cpu_pct = match (self.prev_cpu, sample.cpu) {
            (Some(prev), Some(next)) => cpu_usage_pct(prev, next),
            _ => None,
        };
        if let Some(cpu) = sample.cpu {
            self.prev_cpu = Some(cpu);
        }

        let mut rx_rate = None;
        let mut tx_rate = None;
        if let (Some(rx), Some(tx)) = (sample.net_rx, sample.net_tx) {
            if let Some((prev_rx, prev_tx, prev_ts)) = self.prev_net {
                let dt = (sample.ts_ns.saturating_sub(prev_ts)) as f64 / 1_000_000_000.0;
                rx_rate = net_rate_bytes_per_sec(prev_rx, rx, dt);
                tx_rate = net_rate_bytes_per_sec(prev_tx, tx, dt);
            }
            self.prev_net = Some((rx, tx, sample.ts_ns));
        }

        if sample.docker_present {
            self.last_containers = sample.containers.clone();
        }
        if !sample.raids.is_empty() {
            self.last_raids = sample.raids.clone();
        }
        if let Some(alive) = sample.link_agent {
            self.last_link_agent = Some(alive);
        }

        if let Some(net_out) = sample.net_out {
            if net_out.ok {
                self.internet_fail_streak = 0;
                self.internet.state = InternetState::Up;
                self.internet.latency_ms = net_out.latency_ms;
                self.internet.last_ok_ts = Some(now_unix_ms());
            } else {
                self.internet_fail_streak = self.internet_fail_streak.saturating_add(1);
                if self.internet_fail_streak >= 2 {
                    self.internet.state = InternetState::Down;
                    self.internet.latency_ms = None;
                }
            }
        }
        self.internet.link_agent_alive = self.last_link_agent;

        if link == LinkState::Connected {
            self.last_success_ts = Some(now_unix_ms());
        }

        let mem = sample.mem.map(|m| MemStatus {
            total_kb: Some(m.total_kb),
            available_kb: Some(m.available_kb),
            swap_total_kb: Some(m.swap_total_kb),
            swap_free_kb: Some(m.swap_free_kb),
        });

        MetricsSnapshot {
            ts: now_unix_ms(),
            link,
            last_success_ts: self.last_success_ts,
            internet: self.internet.clone(),
            cpu: CpuStatus {
                usage_pct: cpu_pct,
                load1: sample.load1,
                cores: sample.cores,
            },
            mem: mem.unwrap_or(MemStatus {
                total_kb: None,
                available_kb: None,
                swap_total_kb: None,
                swap_free_kb: None,
            }),
            uptime_sec: sample.uptime_sec,
            net: NetStatus {
                iface: sample.net_iface,
                rx_bytes_per_sec: rx_rate,
                tx_bytes_per_sec: tx_rate,
            },
            disks: sample.disks,
            temps: select_display_temps(&sample.temps),
            raids: self.last_raids.clone(),
            containers: self.last_containers.clone(),
            active_access: Vec::new(),
        }
    }
}

impl Default for Reducer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_block;

    fn two_blocks() -> (RawSample, RawSample) {
        let a = parse_block(
            "=BEGIN 1000000000000000000\n\
=CPU 3792271 2704154 4072774 247797410 2141694 0 115334 0 0 0\n\
=NET eth0 1000000000 500000000\n\
=NET_OUT down\n\
=END\n",
        )
        .unwrap();
        let b = parse_block(
            "=BEGIN 1000000002000000000\n\
=CPU 3792297 2704154 4072794 247798649 2141700 0 115335 0 0 0\n\
=NET eth0 1000593338 500043624\n\
=NET_OUT down\n\
=END\n",
        )
        .unwrap();
        (a, b)
    }

    #[test]
    fn first_frame_rates_are_none() {
        let mut r = Reducer::new();
        let (a, _) = two_blocks();
        let snap = r.apply(a, LinkState::Connected);
        assert!(snap.cpu.usage_pct.is_none());
        assert!(snap.net.rx_bytes_per_sec.is_none());
    }

    #[test]
    fn second_frame_has_real_rates() {
        let mut r = Reducer::new();
        let (a, b) = two_blocks();
        r.apply(a, LinkState::Connected);
        let snap = r.apply(b, LinkState::Connected);
        assert!(snap.cpu.usage_pct.unwrap() > 0.0);
        assert!(snap.net.rx_bytes_per_sec.unwrap() > 0.0);
        // The NAS names its own interface; nothing is hard-coded on our side.
        assert_eq!(snap.net.iface.as_deref(), Some("eth0"));
    }

    #[test]
    fn internet_needs_two_failures_to_go_down() {
        let mut r = Reducer::new();
        let (a, b) = two_blocks();
        let s1 = r.apply(a, LinkState::Connected);
        assert_eq!(s1.internet.state, InternetState::Unknown);
        let s2 = r.apply(b, LinkState::Connected);
        assert_eq!(s2.internet.state, InternetState::Down);
    }

    #[test]
    fn reconnect_clears_rate_state() {
        let mut r = Reducer::new();
        let (a, b) = two_blocks();
        r.apply(a, LinkState::Connected);
        r.apply(b, LinkState::Connected);
        r.reset_rates();
        let (c, _) = two_blocks();
        let snap = r.apply(c, LinkState::Connected);
        assert!(snap.cpu.usage_pct.is_none());
    }
}
