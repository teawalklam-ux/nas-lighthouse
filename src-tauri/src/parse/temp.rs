use crate::model::TempStatus;

#[derive(Debug, Clone)]
pub struct RawTemp {
    pub chip: String,
    pub sensor: String,
    pub milli_c: i64,
}

pub fn parse_temp_line(text: &str) -> Option<RawTemp> {
    let mut parts = text.split_whitespace();
    let chip = parts.next()?.to_string();
    let sensor = parts.next()?.to_string();
    let milli_c = parts.next()?.parse().ok()?;
    Some(RawTemp {
        chip,
        sensor,
        milli_c,
    })
}

/// hwmon chip name → the label the UI looks for. Several chips map to one
/// label so Intel, AMD and ARM boards all land on "CPU": whichever is present
/// wins, in listed order.
const CHIP_LABELS: &[(&str, &str)] = &[
    ("coretemp", "CPU"),
    ("k10temp", "CPU"),
    ("zenpower", "CPU"),
    ("cpu_thermal", "CPU"),
    ("soc_thermal", "CPU"),
    ("nvme", "NVMe"),
    ("drivetemp", "硬盘"),
    ("spd5118", "内存"),
    ("jc42", "内存"),
    ("acpitz", "主板"),
    ("nct6775", "主板"),
    ("it87", "主板"),
];

/// Chips that report something other than a component temperature, or that
/// would crowd out the useful ones.
const SKIP_CHIPS: &[&str] = &["amdgpu", "iwlwifi", "mlxsw", "pch"];

/// Most entries a board can contribute before the overlay gets unreadable.
const MAX_TEMPS: usize = 6;

pub fn select_display_temps(raw: &[RawTemp]) -> Vec<TempStatus> {
    let reading = |chip: &str| -> Option<f64> {
        let of_chip: Vec<_> = raw.iter().filter(|t| t.chip == chip).collect();
        let chosen = of_chip
            .iter()
            .find(|t| t.sensor == "temp1_input")
            .or_else(|| of_chip.first())?;
        Some(chosen.milli_c as f64 / 1000.0)
    };

    let mut out: Vec<TempStatus> = Vec::new();
    for (chip, label) in CHIP_LABELS {
        if out.iter().any(|t| t.label == *label) {
            continue;
        }
        if let Some(celsius) = reading(chip) {
            out.push(TempStatus {
                label: (*label).to_string(),
                celsius,
            });
        }
    }

    // Anything the table does not know about still shows up, under its own chip
    // name, so an unrecognised NAS is not left with an empty temperature row.
    for temp in raw {
        if out.len() >= MAX_TEMPS {
            break;
        }
        let known = CHIP_LABELS.iter().any(|(chip, _)| *chip == temp.chip);
        if known || SKIP_CHIPS.contains(&temp.chip.as_str()) {
            continue;
        }
        if out.iter().any(|t| t.label == temp.chip) {
            continue;
        }
        out.push(TempStatus {
            label: temp.chip.clone(),
            celsius: temp.milli_c as f64 / 1000.0,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(temps: &[TempStatus]) -> Vec<&str> {
        temps.iter().map(|t| t.label.as_str()).collect()
    }

    #[test]
    fn maps_hwmon_by_name_not_index() {
        let raw = vec![
            parse_temp_line("acpitz temp1_input 27800").unwrap(),
            parse_temp_line("nvme temp1_input 39850").unwrap(),
            parse_temp_line("nvme temp2_input 55850").unwrap(),
            parse_temp_line("spd5118 temp1_input 38250").unwrap(),
            parse_temp_line("coretemp temp10_input 40000").unwrap(),
            parse_temp_line("coretemp temp1_input 41000").unwrap(),
            parse_temp_line("coretemp temp2_input 32000").unwrap(),
        ];
        let temps = select_display_temps(&raw);
        assert_eq!(labels(&temps), vec!["CPU", "NVMe", "内存", "主板"]);
        assert!((temps[0].celsius - 41.0).abs() < f64::EPSILON);
        assert!((temps[1].celsius - 39.85).abs() < 0.001);
    }

    #[test]
    fn amd_and_arm_chips_land_on_the_cpu_label() {
        let amd = select_display_temps(&[parse_temp_line("k10temp temp1_input 45000").unwrap()]);
        assert_eq!(labels(&amd), vec!["CPU"]);
        let arm =
            select_display_temps(&[parse_temp_line("cpu_thermal temp1_input 51500").unwrap()]);
        assert_eq!(labels(&arm), vec!["CPU"]);
    }

    #[test]
    fn one_cpu_label_even_with_several_cpu_chips() {
        let temps = select_display_temps(&[
            parse_temp_line("k10temp temp1_input 45000").unwrap(),
            parse_temp_line("coretemp temp1_input 41000").unwrap(),
        ]);
        assert_eq!(labels(&temps), vec!["CPU"]);
        // coretemp is listed first, so it wins.
        assert!((temps[0].celsius - 41.0).abs() < f64::EPSILON);
    }

    #[test]
    fn unknown_chips_still_show_under_their_own_name() {
        let temps = select_display_temps(&[
            parse_temp_line("coretemp temp1_input 41000").unwrap(),
            parse_temp_line("mystery_soc temp1_input 37000").unwrap(),
            parse_temp_line("amdgpu temp1_input 60000").unwrap(),
        ]);
        assert_eq!(labels(&temps), vec!["CPU", "mystery_soc"]);
    }

    #[test]
    fn caps_the_number_of_rows() {
        let raw: Vec<RawTemp> = (0..12)
            .map(|i| parse_temp_line(&format!("chip{i} temp1_input 30000")).unwrap())
            .collect();
        assert_eq!(select_display_temps(&raw).len(), MAX_TEMPS);
    }
}
