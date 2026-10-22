use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LinkState {
    Connected,
    Reconnecting,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InternetState {
    Unknown,
    Up,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InternetStatus {
    pub state: InternetState,
    pub last_ok_ts: Option<u64>,
    pub latency_ms: Option<u32>,
    pub link_agent_alive: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuStatus {
    pub usage_pct: Option<f64>,
    pub load1: Option<f64>,
    pub cores: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemStatus {
    pub total_kb: Option<u64>,
    pub available_kb: Option<u64>,
    pub swap_total_kb: Option<u64>,
    pub swap_free_kb: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetStatus {
    /// Interface the NAS reported on, so the UI can name it instead of
    /// hard-coding one. `None` until the first frame arrives.
    pub iface: Option<String>,
    pub rx_bytes_per_sec: Option<f64>,
    pub tx_bytes_per_sec: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskStatus {
    pub mount: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TempStatus {
    pub label: String,
    pub celsius: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PortMap {
    pub host: u16,
    pub container: u16,
    pub proto: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerStatus {
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub health: Option<String>,
    pub ports: Vec<PortMap>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaidStatus {
    pub name: String,
    pub level: String,
    pub disks_up: u32,
    pub disks_total: u32,
    pub devices: String,
    pub healthy: bool,
    pub recovering: bool,
    pub recovery_pct: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HealthState {
    Unknown,
    Up,
    Degraded,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemHealth {
    pub state: HealthState,
    pub last_ok_ts: Option<u64>,
    pub last_checked_ts: Option<u64>,
    pub changed_ts: Option<u64>,
    pub latency_ms: Option<u32>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EventSeverity {
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthEvent {
    pub id: String,
    pub source: String,
    pub source_id: String,
    pub title: String,
    pub severity: EventSeverity,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub message: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NavItem {
    pub id: String,
    pub title: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    pub url: String,
    pub host: String,
    pub port: u16,
    pub container_name: Option<String>,
    #[serde(default = "default_group")]
    pub group: String,
    #[serde(default)]
    pub order: u32,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default = "default_health_check")]
    pub health_check: bool,
}

fn default_icon() -> String {
    "box".to_string()
}

fn default_group() -> String {
    "NAS 服务".to_string()
}

fn default_health_check() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CandidateSource {
    Docker,
    Access,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub host: String,
    pub port: u16,
    pub suggested_title: String,
    pub url: String,
    pub container_name: Option<String>,
    pub source: CandidateSource,
    pub proto: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveAccess {
    pub port: u16,
    pub nav_item_id: Option<String>,
    pub title: Option<String>,
    pub container_name: Option<String>,
    pub connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSnapshot {
    pub ts: u64,
    pub link: LinkState,
    pub last_success_ts: Option<u64>,
    pub internet: InternetStatus,
    pub cpu: CpuStatus,
    pub mem: MemStatus,
    pub uptime_sec: Option<f64>,
    pub net: NetStatus,
    pub disks: Vec<DiskStatus>,
    pub temps: Vec<TempStatus>,
    pub raids: Vec<RaidStatus>,
    pub containers: Vec<ContainerStatus>,
    pub active_access: Vec<ActiveAccess>,
}

impl MetricsSnapshot {
    pub fn empty() -> Self {
        Self {
            ts: now_unix_ms(),
            link: LinkState::Reconnecting,
            last_success_ts: None,
            internet: InternetStatus {
                state: InternetState::Unknown,
                last_ok_ts: None,
                latency_ms: None,
                link_agent_alive: None,
            },
            cpu: CpuStatus {
                usage_pct: None,
                load1: None,
                cores: 0,
            },
            mem: MemStatus {
                total_kb: None,
                available_kb: None,
                swap_total_kb: None,
                swap_free_kb: None,
            },
            uptime_sec: None,
            net: NetStatus {
                iface: None,
                rx_bytes_per_sec: None,
                tx_bytes_per_sec: None,
            },
            disks: Vec::new(),
            temps: Vec::new(),
            raids: Vec::new(),
            containers: Vec::new(),
            active_access: Vec::new(),
        }
    }
}

pub fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
