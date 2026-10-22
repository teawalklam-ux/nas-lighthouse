#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuTimes {
    pub idle: u64,
    pub total: u64,
}

/// `/proc/stat` 的 `cpu` 行：user nice system idle iowait irq softirq steal guest guest_nice
/// guest / guest_nice 已包含在 user / nice 中，合计只取前 8 项。
pub fn parse_cpu_fields(fields: &[u64]) -> Option<CpuTimes> {
    if fields.len() < 5 {
        return None;
    }
    let idle = fields[3].saturating_add(*fields.get(4).unwrap_or(&0));
    let total: u64 = fields.iter().take(8).copied().sum();
    if total == 0 {
        return None;
    }
    Some(CpuTimes { idle, total })
}

#[cfg(test)]
pub fn parse_cpu_text(text: &str) -> Option<CpuTimes> {
    let text = text.trim();
    let rest = text
        .strip_prefix("cpu ")
        .or_else(|| text.strip_prefix("cpu\t"))
        .or_else(|| text.strip_prefix("=CPU "))
        .unwrap_or(text);
    let nums: Vec<u64> = rest
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();
    parse_cpu_fields(&nums)
}

pub fn cpu_usage_pct(prev: CpuTimes, next: CpuTimes) -> Option<f64> {
    let d_total = next.total.saturating_sub(prev.total);
    let d_idle = next.idle.saturating_sub(prev.idle);
    if d_total == 0 {
        return None;
    }
    let busy = d_total.saturating_sub(d_idle) as f64;
    let pct = (busy / d_total as f64) * 100.0;
    Some(pct.clamp(0.0, 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Synthetic /proc/stat pair ~2 seconds apart.
    const STAT1: &str = "cpu  3792271 2704154 4072774 247797410 2141694 0 115334 0 0 0";
    const STAT2: &str = "cpu  3792297 2704154 4072794 247798649 2141700 0 115335 0 0 0";

    #[test]
    fn parses_real_stat_line() {
        let a = parse_cpu_text(STAT1).unwrap();
        assert!(a.total > a.idle);
        assert_eq!(a.idle, 247797410 + 2141694);
    }

    #[test]
    fn usage_from_real_delta_is_small_not_zero_not_fake() {
        let a = parse_cpu_text(STAT1).unwrap();
        let b = parse_cpu_text(STAT2).unwrap();
        let pct = cpu_usage_pct(a, b).unwrap();
        assert!(pct > 0.0, "idle-only would be 0, this sample had some work");
        assert!(pct < 20.0, "NAS was mostly idle, got {pct}");
        let d_total = b.total - a.total;
        let d_idle = b.idle - a.idle;
        let expected = (1.0 - d_idle as f64 / d_total as f64) * 100.0;
        assert!((pct - expected).abs() < 1e-9);
    }

    #[test]
    fn first_frame_cannot_invent_a_rate() {
        assert!(cpu_usage_pct(
            parse_cpu_text(STAT1).unwrap(),
            parse_cpu_text(STAT1).unwrap()
        )
        .is_none());
    }
}
