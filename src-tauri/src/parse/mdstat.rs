use crate::model::RaidStatus;

fn bracket_groups(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in s.char_indices() {
        if c == '[' {
            start = Some(i + 1);
        } else if c == ']' {
            if let Some(st) = start {
                out.push(&s[st..i]);
                start = None;
            }
        }
    }
    out
}

pub fn parse_mdstat(text: &str) -> Vec<RaidStatus> {
    let mut raids = Vec::new();
    let mut current: Option<RaidStatus> = None;
    let mut recovering = false;
    let mut recovery_pct = None;

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("md") && line.contains(':') {
            if let Some(done) = current.take() {
                raids.push(done);
            }
            recovering = false;
            recovery_pct = None;
            let mut parts = line.split_whitespace();
            let name = parts.next().unwrap_or("md").to_string();
            let level = parts
                .find(|p| p.starts_with("raid"))
                .unwrap_or("unknown")
                .to_string();
            current = Some(RaidStatus {
                name,
                level,
                disks_up: 0,
                disks_total: 0,
                devices: String::new(),
                healthy: false,
                recovering: false,
                recovery_pct: None,
            });
            continue;
        }

        let Some(raid) = current.as_mut() else {
            continue;
        };

        if line.contains("recovery") || line.contains("resync") || line.contains("reshape") {
            recovering = true;
            if let Some(idx) = line.find('%') {
                let start = line[..idx]
                    .rfind(|c: char| !(c.is_ascii_digit() || c == '.'))
                    .map(|i| i + 1)
                    .unwrap_or(0);
                recovery_pct = line[start..idx].parse().ok();
            }
        }

        if let Some(level_idx) = line.find("level ") {
            let after = line[level_idx + 6..].split_whitespace().next();
            if let Some(n) = after {
                if raid.level == "unknown" || raid.level.is_empty() {
                    raid.level = format!("raid{n}");
                }
            }
        }

        for group in bracket_groups(line) {
            if let Some((up, total)) = group.split_once('/') {
                if let (Ok(u), Ok(t)) = (up.parse::<u32>(), total.parse::<u32>()) {
                    raid.disks_up = u;
                    raid.disks_total = t;
                }
            } else if !group.is_empty() && group.chars().all(|c| c == 'U' || c == '_' || c == '.') {
                raid.devices = group.to_string();
            }
        }

        raid.recovering = recovering;
        raid.recovery_pct = recovery_pct;
        raid.healthy = !raid.recovering
            && raid.disks_total > 0
            && raid.disks_up == raid.disks_total
            && !raid.devices.contains('_');
    }

    if let Some(done) = current {
        raids.push(done);
    }
    raids
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEALTHY: &str = "\
Personalities : [raid0] [raid1] [raid4] [raid5] [raid6] [raid10]
md1 : active raid5 sda2[0] sdc2[3] sdd2[2] sdb2[1]
      46829239296 blocks super 1.2 level 5, 64k chunk, algorithm 2 [4/4] [UUUU]
      bitmap: 12/117 pages [48KB], 65536KB chunk

unused devices: <none>
";

    const DEGRADED: &str = "\
md1 : active raid5 sda2[0] sdc2[3] sdd2[2] sdb2[1]
      46829239296 blocks super 1.2 level 5, 64k chunk, algorithm 2 [3/4] [UU_U]
      [====>................]  recovery = 21.2% (123/456) finish=100min speed=100K/sec
";

    #[test]
    fn parses_real_raid5_healthy() {
        let raids = parse_mdstat(HEALTHY);
        assert_eq!(raids.len(), 1);
        let r = &raids[0];
        assert_eq!(r.name, "md1");
        assert_eq!(r.level, "raid5");
        assert_eq!(r.disks_up, 4);
        assert_eq!(r.disks_total, 4);
        assert_eq!(r.devices, "UUUU");
        assert!(r.healthy);
        assert!(!r.recovering);
    }

    #[test]
    fn parses_degraded_rebuild() {
        let r = &parse_mdstat(DEGRADED)[0];
        assert!(!r.healthy);
        assert!(r.recovering);
        assert_eq!(r.disks_up, 3);
        assert_eq!(r.disks_total, 4);
        assert_eq!(r.devices, "UU_U");
        assert_eq!(r.recovery_pct, Some(21.2));
    }
}
