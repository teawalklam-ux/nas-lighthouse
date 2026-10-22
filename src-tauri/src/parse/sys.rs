use crate::model::DiskStatus;

#[derive(Debug, Clone, Copy)]
pub struct MemRaw {
    pub total_kb: u64,
    pub available_kb: u64,
    pub swap_total_kb: u64,
    pub swap_free_kb: u64,
}

pub fn parse_mem_fields(fields: &[u64]) -> Option<MemRaw> {
    if fields.len() < 4 {
        return None;
    }
    Some(MemRaw {
        total_kb: fields[0],
        available_kb: fields[1],
        swap_total_kb: fields[2],
        swap_free_kb: fields[3],
    })
}

#[cfg(test)]
pub fn parse_meminfo(text: &str) -> Option<MemRaw> {
    let mut total = None;
    let mut available = None;
    let mut swap_total = None;
    let mut swap_free = None;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let key = parts.next().unwrap_or("");
        let value = parts.next().and_then(|v| v.parse::<u64>().ok());
        match key {
            "MemTotal:" => total = value,
            "MemAvailable:" => available = value,
            "SwapTotal:" => swap_total = value,
            "SwapFree:" => swap_free = value,
            _ => {}
        }
    }
    Some(MemRaw {
        total_kb: total?,
        available_kb: available?,
        swap_total_kb: swap_total?,
        swap_free_kb: swap_free?,
    })
}

pub fn parse_load1(text: &str) -> Option<f64> {
    text.split_whitespace().next()?.parse().ok()
}

pub fn parse_uptime(text: &str) -> Option<f64> {
    text.split_whitespace().next()?.parse().ok()
}

#[cfg(test)]
pub fn parse_netdev_iface(text: &str, iface: &str) -> Option<(u64, u64)> {
    let prefix = format!("{iface}:");
    for line in text.lines() {
        let line = line.trim();
        let rest = if let Some(r) = line.strip_prefix(&prefix) {
            r
        } else if line.starts_with(iface) {
            line.split_once(':')?.1
        } else {
            continue;
        };
        let cols: Vec<&str> = rest.split_whitespace().collect();
        if cols.len() < 9 {
            continue;
        }
        let rx = cols[0].parse().ok()?;
        let tx = cols[8].parse().ok()?;
        return Some((rx, tx));
    }
    None
}

#[cfg(test)]
pub fn parse_net_compact(text: &str) -> Option<(String, u64, u64)> {
    let mut parts = text.split_whitespace();
    let iface = parts.next()?.to_string();
    let rx = parts.next()?.parse().ok()?;
    let tx = parts.next()?.parse().ok()?;
    Some((iface, rx, tx))
}

pub fn net_rate_bytes_per_sec(prev: u64, next: u64, dt_sec: f64) -> Option<f64> {
    if dt_sec <= 0.0 || next < prev {
        return None;
    }
    Some((next - prev) as f64 / dt_sec)
}

pub fn parse_disk_line(text: &str) -> Option<DiskStatus> {
    let text = text.trim();
    // New frames: total used mount. Accept older recorded frames as well.
    if text.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        let (total, rest) = text.split_once(char::is_whitespace)?;
        let (used, mount) = rest.trim_start().split_once(char::is_whitespace)?;
        Some(DiskStatus {
            mount: mount.trim().to_string(),
            total_bytes: total.parse().ok()?,
            used_bytes: used.parse().ok()?,
        })
    } else {
        let mut parts = text.split_whitespace();
        Some(DiskStatus {
            mount: parts.next()?.to_string(),
            total_bytes: parts.next()?.parse().ok()?,
            used_bytes: parts.next()?.parse().ok()?,
        })
    }
}

#[cfg(test)]
pub fn parse_df_p(text: &str) -> Vec<DiskStatus> {
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 6 {
            continue;
        }
        let mount = cols[5];
        if mount == "/"
            || mount.starts_with("/proc")
            || mount.starts_with("/sys")
            || mount.starts_with("/dev")
            || mount.starts_with("/run")
            || mount.starts_with("/boot")
            || mount.starts_with("/rootfs")
        {
            continue;
        }
        let Some(total_bytes) = cols[1].parse().ok() else {
            continue;
        };
        let Some(used_bytes) = cols[2].parse().ok() else {
            continue;
        };
        out.push(DiskStatus {
            mount: mount.to_string(),
            total_bytes,
            used_bytes,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_frames_preserve_mount_spaces_and_accept_old_frames() {
        let disk = parse_disk_line("1000 250 /mnt/Family Photos").unwrap();
        assert_eq!(disk.mount, "/mnt/Family Photos");
        assert_eq!(disk.used_bytes, 250);
        assert_eq!(
            parse_disk_line("/mnt/data 1000 250").unwrap().total_bytes,
            1000
        );
        assert!(parse_disk_line("bad frame").is_none());
    }

    #[test]
    fn meminfo_uses_available_not_free() {
        let text = "\
MemTotal:       16142096 kB
MemFree:         2331676 kB
MemAvailable:   10001148 kB
SwapTotal:      14360552 kB
SwapFree:       13355044 kB
";
        let m = parse_meminfo(text).unwrap();
        assert_eq!(m.total_kb, 16_142_096);
        assert_eq!(m.available_kb, 10_001_148);
        assert_ne!(m.available_kb, 2_331_676);
    }

    #[test]
    fn netdev_reads_named_iface_not_veth() {
        let text = "\
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
  eth0: 1000000000 1000    0    0    0     0          0   0 500000000 800    0  0    0     0       0          0
 bond0: 2000000000 2000    0    0    0     0          0   0 900000000 900    0  0    0     0       0          0
";
        let (rx, tx) = parse_netdev_iface(text, "bond0").unwrap();
        assert_eq!(rx, 2_000_000_000);
        assert_eq!(tx, 900_000_000);
    }

    #[test]
    fn net_rate_from_byte_delta() {
        let rx1 = 1_000_000_000_u64;
        let tx1 = 500_000_000_u64;
        let rx2 = 1_000_593_338_u64;
        let tx2 = 500_043_624_u64;
        let rx = net_rate_bytes_per_sec(rx1, rx2, 2.0).unwrap();
        let tx = net_rate_bytes_per_sec(tx1, tx2, 2.0).unwrap();
        assert!((rx - 296_669.0).abs() < 1.0);
        assert!((tx - 21_812.0).abs() < 1.0);
        assert!(net_rate_bytes_per_sec(rx2, rx1, 2.0).is_none());
        assert!(net_rate_bytes_per_sec(rx1, rx2, 0.0).is_none());
    }

    #[test]
    fn df_skips_system_mounts() {
        let text = "\
Filesystem                                           1-blocks           Used      Available Capacity Mounted on
/dev/mapper/vg0-data                               4000000000000 1000000000000 3000000000000      25% /mnt/data
/dev/nvme0n1p2                                      120000000000     8000000000   112000000000       7% /srv
/dev/loop0                                             792723456      792723456              0     100% /rootfs/base
";
        let disks = parse_df_p(text);
        assert_eq!(disks.len(), 2);
        assert_eq!(disks[0].mount, "/mnt/data");
        assert_eq!(disks[0].total_bytes, 4_000_000_000_000);
        assert_eq!(disks[1].mount, "/srv");
        let pct = disks[0].used_bytes as f64 / disks[0].total_bytes as f64;
        assert!((pct - 0.25).abs() < 0.02);
    }
}
