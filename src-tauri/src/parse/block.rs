use crate::model::DiskStatus;
use crate::model::{ContainerStatus, RaidStatus};
use crate::parse::cpu::{parse_cpu_fields, CpuTimes};
use crate::parse::docker::parse_docker_line;
use crate::parse::mdstat::parse_mdstat;
use crate::parse::sys::{parse_load1, parse_mem_fields, parse_uptime, MemRaw};
use crate::parse::temp::{parse_temp_line, RawTemp};

#[derive(Debug, Clone)]
pub struct NetOutSample {
    pub ok: bool,
    pub latency_ms: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct RawSample {
    pub ts_ns: u128,
    pub cpu: Option<CpuTimes>,
    pub mem: Option<MemRaw>,
    pub load1: Option<f64>,
    pub cores: u32,
    pub uptime_sec: Option<f64>,
    pub net_iface: Option<String>,
    pub net_rx: Option<u64>,
    pub net_tx: Option<u64>,
    pub disks: Vec<DiskStatus>,
    pub temps: Vec<RawTemp>,
    pub raids: Vec<RaidStatus>,
    pub containers: Vec<ContainerStatus>,
    pub docker_present: bool,
    pub net_out: Option<NetOutSample>,
    pub link_agent: Option<bool>,
}

pub fn parse_ts_ns(text: &str) -> Option<u128> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Some((sec, frac)) = text.split_once('.') {
        let sec: u128 = sec.parse().ok()?;
        let mut digits: String = frac
            .chars()
            .filter(|c| c.is_ascii_digit())
            .take(9)
            .collect();
        while digits.len() < 9 {
            digits.push('0');
        }
        let nsec: u128 = digits.parse().ok()?;
        return Some(sec.saturating_mul(1_000_000_000) + nsec);
    }
    let n: u128 = text.parse().ok()?;
    Some(if n < 10_000_000_000 {
        n.saturating_mul(1_000_000_000)
    } else if n < 10_000_000_000_000 {
        n.saturating_mul(1_000_000)
    } else {
        n
    })
}

pub fn parse_block(text: &str) -> Option<RawSample> {
    let text = text.replace('\r', "");
    let mut sample = RawSample::default();
    let mut mdstat = String::new();
    let mut saw_begin = false;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("=BEGIN ") {
            sample.ts_ns = parse_ts_ns(rest)?;
            saw_begin = true;
            continue;
        }
        if line == "=END" {
            break;
        }
        if let Some(rest) = line.strip_prefix("=CPU ") {
            let nums: Vec<u64> = rest
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            sample.cpu = parse_cpu_fields(&nums);
        } else if let Some(rest) = line.strip_prefix("=MEM ") {
            let nums: Vec<u64> = rest
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            sample.mem = parse_mem_fields(&nums);
        } else if let Some(rest) = line.strip_prefix("=LOAD ") {
            sample.load1 = parse_load1(rest);
        } else if let Some(rest) = line.strip_prefix("=CORES ") {
            if let Some(n) = rest.split_whitespace().next().and_then(|s| s.parse().ok()) {
                sample.cores = n;
            }
        } else if let Some(rest) = line.strip_prefix("=UPTIME ") {
            sample.uptime_sec = parse_uptime(rest);
        } else if let Some(rest) = line.strip_prefix("=NET ") {
            let mut parts = rest.split_whitespace();
            if let Some(iface) = parts.next() {
                sample.net_iface = Some(iface.to_string());
            }
            sample.net_rx = parts.next().and_then(|s| s.parse().ok());
            sample.net_tx = parts.next().and_then(|s| s.parse().ok());
        } else if let Some(rest) = line.strip_prefix("=DISK ") {
            if let Some(disk) = crate::parse::sys::parse_disk_line(rest) {
                sample.disks.push(disk);
            }
        } else if let Some(rest) = line.strip_prefix("=TEMP ") {
            if let Some(temp) = parse_temp_line(rest) {
                sample.temps.push(temp);
            }
        } else if let Some(rest) = line.strip_prefix("=MDSTAT ") {
            mdstat.push_str(&rest.replace('|', "\n"));
            mdstat.push('\n');
        } else if let Some(rest) = line.strip_prefix("=DOCKER ") {
            sample.docker_present = true;
            if let Some(c) = parse_docker_line(rest) {
                sample.containers.push(c);
            }
        } else if let Some(rest) = line.strip_prefix("=NET_OUT ") {
            let mut parts = rest.split_whitespace();
            let status = parts.next().unwrap_or("");
            if status == "ok" {
                sample.net_out = Some(NetOutSample {
                    ok: true,
                    latency_ms: parts.next().and_then(|s| s.parse().ok()),
                });
            } else {
                sample.net_out = Some(NetOutSample {
                    ok: false,
                    latency_ms: None,
                });
            }
        } else if let Some(rest) = line.strip_prefix("=LINK_AGENT ") {
            sample.link_agent = Some(rest.trim() == "1");
        }
    }

    if !saw_begin {
        return None;
    }
    if !mdstat.is_empty() {
        sample.raids = parse_mdstat(&mdstat);
    }
    Some(sample)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK: &str = r#"=BEGIN 1756870000123456789
=CPU 3792271 2704154 4072774 247797410 2141694 0 115334 0 0 0
=MEM 16384000 10000000 8388608 8000000
=LOAD 0.20 0.32 0.54
=CORES 8
=UPTIME 440130.24
=NET eth0 1000000000 500000000
=DISK /mnt/data 4000000000000 1000000000000
=DISK / 120000000000 8000000000
=TEMP coretemp temp1_input 41000
=TEMP nvme temp1_input 39850
=TEMP spd5118 temp1_input 38250
=TEMP acpitz temp1_input 27800
=MDSTAT Personalities : [raid0] [raid1]
=MDSTAT md1 : active raid5 sda2[0] sdc2[3] sdd2[2] sdb2[1]
=MDSTAT       46829239296 blocks super 1.2 level 5, 64k chunk, algorithm 2 [4/4] [UUUU]
=DOCKER {"Names":"media-jellyfin-media-jellyfin-1","Image":"jellyfin/jellyfin","State":"running","Status":"Up 2 hours (healthy)","HealthStatus":"healthy","Ports":"0.0.0.0:8096->8096/tcp, [::]:8096->8096/tcp"}
=NET_OUT ok 8
=LINK_AGENT 1
=END
"#;

    #[test]
    fn parses_full_block() {
        let s = parse_block(BLOCK).unwrap();
        assert!(s.ts_ns > 0);
        assert!(s.cpu.is_some());
        assert_eq!(s.cores, 8);
        assert_eq!(s.net_iface.as_deref(), Some("eth0"));
        assert_eq!(s.disks.len(), 2);
        assert_eq!(s.raids[0].name, "md1");
        assert!(s.raids[0].healthy);
        assert_eq!(s.containers.len(), 1);
        assert_eq!(s.net_out.unwrap().latency_ms, Some(8));
        assert_eq!(s.link_agent, Some(true));
    }

    #[test]
    fn rejects_block_without_begin() {
        assert!(parse_block("=CPU 1 2 3 4 5\n=END\n").is_none());
    }
}
