use crate::model::NavItem;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// Bumped whenever a released config gains fields that need a rewrite on load.
pub const SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayDisplay {
    #[serde(default = "default_true")]
    pub show_labels: bool,
    #[serde(default = "default_precision")]
    pub precision: u8,
    #[serde(default = "default_true")]
    pub show_link: bool,
    #[serde(default = "default_true")]
    pub show_internet: bool,
    #[serde(default = "default_true")]
    pub show_cpu: bool,
    #[serde(default = "default_true")]
    pub show_cpu_temp: bool,
    #[serde(default = "default_true")]
    pub show_mem: bool,
    #[serde(default)]
    pub show_mem_temp: bool,
    #[serde(default = "default_true")]
    pub show_net: bool,
    #[serde(default = "default_true")]
    pub show_access: bool,
    #[serde(default = "default_true")]
    pub show_ports: bool,
    #[serde(default = "default_true")]
    pub snap_hide: bool,
    #[serde(default)]
    pub opaque: bool,
}

fn default_precision() -> u8 {
    1
}

impl Default for OverlayDisplay {
    fn default() -> Self {
        Self {
            show_labels: true,
            precision: 1,
            show_link: true,
            show_internet: true,
            show_cpu: true,
            show_cpu_temp: true,
            show_mem: true,
            show_mem_temp: false,
            show_net: true,
            show_access: true,
            show_ports: true,
            snap_hide: true,
            opaque: false,
        }
    }
}

impl OverlayDisplay {
    pub fn clamp(&mut self) {
        if self.precision > 2 {
            self.precision = 2;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayConfig {
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub dock: Option<String>,
    #[serde(default)]
    pub display: OverlayDisplay,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            collapsed: false,
            visible: true,
            dock: None,
            display: OverlayDisplay::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiConfig {
    #[serde(default = "default_interval")]
    pub interval_ms: u64,
    #[serde(default = "default_true")]
    pub internet_probe_enabled: bool,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub sidebar_collapsed: bool,
    #[serde(default)]
    pub nav_compact: bool,
    #[serde(default = "default_bloub_pin")]
    pub bloub_pin: String,
    #[serde(default)]
    pub overlay: OverlayConfig,
}

fn default_interval() -> u64 {
    2000
}

fn default_true() -> bool {
    true
}

fn default_bloub_pin() -> String {
    "auto".into()
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            interval_ms: default_interval(),
            internet_probe_enabled: true,
            autostart: false,
            sidebar_collapsed: false,
            nav_compact: false,
            bloub_pin: default_bloub_pin(),
            overlay: OverlayConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlistBackend {
    #[default]
    Alist,
    Openlist,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchConfig {
    #[serde(default = "default_everything_http")]
    pub everything_http: String,
    /// Base URL of an AList / OpenList instance on the NAS. Empty skips NAS
    /// search and cloud-drive mounting.
    #[serde(default)]
    pub alist_url: String,
    /// Container `docker exec` reads the admin token from.
    #[serde(default = "default_alist_container")]
    pub alist_container: String,
    #[serde(default)]
    pub alist_backend: AlistBackend,
    #[serde(default = "default_quark_mount")]
    pub quark_mount: String,
    #[serde(default = "default_baidu_mount")]
    pub baidu_mount: String,
    #[serde(default)]
    pub everything_executable: String,
}

fn default_quark_mount() -> String {
    "/夸克".into()
}
fn default_baidu_mount() -> String {
    "/百度".into()
}

fn default_everything_http() -> String {
    "http://127.0.0.1:80".into()
}

fn default_alist_container() -> String {
    "alist".into()
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            everything_http: default_everything_http(),
            alist_url: String::new(),
            alist_container: default_alist_container(),
            alist_backend: AlistBackend::default(),
            quark_mount: default_quark_mount(),
            baidu_mount: default_baidu_mount(),
            everything_executable: String::new(),
        }
    }
}

/// Everything the remote collector script needs that differs between NAS
/// models and distros. Empty or `None` means "detect it on the NAS".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectorConfig {
    /// Interface to read from `/proc/net/dev`, e.g. `bond0`, `eth0`, `enp2s0`.
    /// `None` follows the default route.
    #[serde(default)]
    pub net_iface: Option<String>,
    /// Mount points to report. Empty lists block-backed filesystems and skips
    /// system and container mounts.
    #[serde(default)]
    pub disk_mounts: Vec<String>,
    /// Hosts tried on TCP 443 to tell whether the NAS can reach the internet.
    #[serde(default = "default_probe_hosts")]
    pub probe_hosts: Vec<String>,
    /// Process name of a vendor remote-access agent — `nas_agent_servi` for
    /// UGREEN Link, `synorelayd` for Synology QuickConnect. `None` skips the
    /// check and reports the channel as unknown.
    #[serde(default)]
    pub link_agent_process: Option<String>,
}

fn default_probe_hosts() -> Vec<String> {
    vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()]
}

impl Default for CollectorConfig {
    fn default() -> Self {
        Self {
            net_iface: None,
            disk_mounts: Vec::new(),
            probe_hosts: default_probe_hosts(),
            link_agent_process: None,
        }
    }
}

impl CollectorConfig {
    pub fn sanitize(&mut self) {
        fn clean(value: Option<String>) -> Option<String> {
            value
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        }
        fn clean_list(list: &mut Vec<String>) {
            for entry in list.iter_mut() {
                *entry = entry.trim().to_string();
            }
            list.retain(|entry| !entry.is_empty());
        }
        self.net_iface = clean(self.net_iface.take());
        self.link_agent_process = clean(self.link_agent_process.take());
        clean_list(&mut self.disk_mounts);
        clean_list(&mut self.probe_hosts);
    }
}

/// Substring of a container name → display name. Ships empty: naming someone's
/// private services is their business, not the repo's.
pub type TitleRules = BTreeMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default = "default_schema")]
    pub schema_version: u32,
    /// `Host` entry from the user's SSH config, or `user@host`. Empty until the
    /// first run is configured.
    #[serde(default)]
    pub ssh_alias: String,
    /// Address the nav page builds service URLs from. Empty until configured.
    #[serde(default)]
    pub nas_host: String,
    #[serde(default)]
    pub nav_items: Vec<NavItem>,
    #[serde(default)]
    pub overview_container_names: Vec<String>,
    /// Extra NAS ports to treat as "being accessed" even before Docker reports
    /// them, e.g. a reverse proxy that owns no container.
    #[serde(default)]
    pub watch_ports: Vec<u16>,
    #[serde(default)]
    pub container_titles: TitleRules,
    #[serde(default)]
    pub collector: CollectorConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub search: SearchConfig,
}

fn default_schema() -> u32 {
    SCHEMA_VERSION
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            ssh_alias: String::new(),
            nas_host: String::new(),
            nav_items: Vec::new(),
            overview_container_names: Vec::new(),
            watch_ports: Vec::new(),
            container_titles: TitleRules::new(),
            collector: CollectorConfig::default(),
            ui: UiConfig::default(),
            search: SearchConfig::default(),
        }
    }
}

impl AppConfig {
    /// False until the user has said how to reach their NAS. The UI shows a
    /// setup prompt instead of an empty dashboard.
    pub fn configured(&self) -> bool {
        !self.ssh_alias.trim().is_empty() && !self.nas_host.trim().is_empty()
    }

    pub fn replace_overview_containers(&mut self, names: Vec<String>) {
        let mut seen = HashSet::new();
        self.overview_container_names = names
            .into_iter()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty() && seen.insert(name.clone()))
            .collect();
    }
}

const APP_DIR: &str = "nas-lighthouse";

fn env_dir(key: &str) -> Option<PathBuf> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Per-user config root. Falls back to the working directory so the app still
/// starts in a stripped-down environment with no HOME or APPDATA.
fn config_root() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(dir) = env_dir("APPDATA") {
            return dir;
        }
        if let Some(dir) = env_dir("USERPROFILE") {
            return dir.join("AppData").join("Roaming");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(dir) = env_dir("HOME") {
            return dir.join("Library").join("Application Support");
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(dir) = env_dir("XDG_CONFIG_HOME") {
            return dir;
        }
        if let Some(dir) = env_dir("HOME") {
            return dir.join(".config");
        }
    }
    PathBuf::from(".")
}

pub fn config_dir() -> PathBuf {
    if let Some(dir) = env_dir("NAS_LIGHTHOUSE_CONFIG_DIR") {
        return dir;
    }
    let root = config_root();
    if root == Path::new(".") {
        return root.join(format!(".{APP_DIR}"));
    }
    root.join(APP_DIR)
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn load() -> anyhow::Result<AppConfig> {
    load_from(&config_path())
}

fn load_from(path: &Path) -> anyhow::Result<AppConfig> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // No implicit access to an older application's private configuration.
            return Ok(AppConfig::default());
        }
        Err(err) => return Err(err.into()),
    };
    let mut cfg: AppConfig = serde_json::from_str(&text)?;
    if cfg.schema_version > SCHEMA_VERSION {
        anyhow::bail!("配置来自较新版本，请升级应用后再打开");
    }
    cfg.collector.sanitize();
    cfg.schema_version = SCHEMA_VERSION;
    Ok(cfg)
}

pub fn save(cfg: &AppConfig) -> anyhow::Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("config.json");
    let tmp = dir.join("config.json.tmp");
    let json = serde_json::to_string_pretty(cfg)?;
    std::fs::write(&tmp, json)?;
    replace_atomic(&tmp, &path)?;
    Ok(())
}

pub(crate) fn replace_atomic(tmp: &Path, dest: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        const MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
        #[link(name = "kernel32")]
        extern "system" {
            fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
        }
        fn wide(path: &Path) -> Vec<u16> {
            path.as_os_str().encode_wide().chain(Some(0)).collect()
        }
        let from = wide(tmp);
        let to = wide(dest);
        // SAFETY: from/to 是带 NUL 的宽字符串，在本次调用期间保持有效。
        let ok = unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_REPLACE_EXISTING) };
        if ok == 0 {
            return Err(std::io::Error::last_os_error());
        }
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(tmp, dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_config_does_not_import_sibling_legacy_configuration() {
        let dir = std::env::temp_dir().join(format!(
            "nas-lighthouse-config-test-{}-{}",
            std::process::id(),
            crate::model::now_unix_ms()
        ));
        let legacy = dir.join("Joo-navi");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(
            legacy.join("config.json"),
            r#"{"sshAlias":"legacy-nas","nasHost":"192.0.2.10"}"#,
        )
        .unwrap();
        let path = dir.join("nas-lighthouse/config.json");
        let cfg = load_from(&path).unwrap();
        assert!(!cfg.configured());
        assert!(!path.exists());
        std::fs::write(legacy.join("config.json"), "broken").unwrap();
        assert!(load_from(&legacy.join("config.json")).is_err());
        std::fs::remove_file(legacy.join("config.json")).unwrap();
        std::fs::remove_dir(&legacy).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }

    #[test]
    fn roundtrip_keeps_nav_item() {
        let mut cfg = AppConfig::default();
        cfg.nas_host = "192.0.2.10".into();
        cfg.nav_items.push(NavItem {
            id: "n1".into(),
            title: "Jellyfin".into(),
            icon: "video".into(),
            url: "http://192.0.2.10:8096/".into(),
            host: "192.0.2.10".into(),
            port: 8096,
            container_name: Some("media-jellyfin-1".into()),
            group: "NAS 服务".into(),
            order: 1,
            favorite: true,
            health_check: true,
        });
        let json = serde_json::to_string(&cfg).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.nav_items[0].title, "Jellyfin");
        assert_eq!(back.nas_host, "192.0.2.10");
        assert!(!json.to_lowercase().contains("password"));
        assert!(!json.to_lowercase().contains("token"));
        assert!(back.overview_container_names.is_empty());
        assert!(!back.ui.autostart);
        assert!(back.ui.overlay.visible);
        assert!(!back.ui.overlay.collapsed);
        assert!(back.ui.overlay.display.show_cpu);
        assert!(back.ui.overlay.display.snap_hide);
        assert!(!back.ui.sidebar_collapsed);
        assert!(!back.ui.nav_compact);
        assert_eq!(back.ui.bloub_pin, "auto");
        assert_eq!(back.search.alist_container, "alist");
        assert!(back.search.alist_url.is_empty());
    }

    #[test]
    fn replace_overview_containers_trims_and_dedupes() {
        let mut cfg = AppConfig::default();
        cfg.replace_overview_containers(vec![
            "  a  ".into(),
            "".into(),
            "a".into(),
            "b".into(),
            "   ".into(),
        ]);
        assert_eq!(cfg.overview_container_names, vec!["a", "b"]);
        cfg.replace_overview_containers(Vec::new());
        assert!(cfg.overview_container_names.is_empty());
    }

    #[test]
    fn default_ships_no_host_and_asks_for_setup() {
        let cfg = AppConfig::default();
        assert!(!cfg.configured());
        assert!(cfg.ssh_alias.is_empty());
        assert!(cfg.nas_host.is_empty());
        assert!(cfg.collector.net_iface.is_none());
        assert!(cfg.collector.disk_mounts.is_empty());
        assert!(cfg.collector.link_agent_process.is_none());
        assert!(cfg.container_titles.is_empty());
        assert!(cfg.watch_ports.is_empty());
        assert!(cfg.search.alist_url.is_empty());
        assert_eq!(cfg.collector.probe_hosts, vec!["1.1.1.1", "8.8.8.8"]);
    }

    #[test]
    fn schema_v1_config_gains_v2_defaults() {
        let json = r#"{"schemaVersion":1,"sshAlias":"nas","nasHost":"192.0.2.10","navItems":[],"ui":{},"search":{"alistUrl":"http://192.0.2.10:5244","alistContainer":"my-alist"}}"#;
        let cfg: AppConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.configured());
        assert_eq!(cfg.collector.probe_hosts, vec!["1.1.1.1", "8.8.8.8"]);
        assert!(cfg.collector.net_iface.is_none());
        assert!(cfg.watch_ports.is_empty());
        assert_eq!(cfg.search.alist_url, "http://192.0.2.10:5244");
        assert_eq!(cfg.search.alist_container, "my-alist");
    }

    #[test]
    fn collector_sanitize_drops_blanks() {
        let mut collector = CollectorConfig {
            net_iface: Some("  ".into()),
            disk_mounts: vec![" /mnt/data ".into(), "".into()],
            probe_hosts: vec!["  1.1.1.1 ".into(), "   ".into()],
            link_agent_process: Some(" synorelayd ".into()),
        };
        collector.sanitize();
        assert!(collector.net_iface.is_none());
        assert_eq!(collector.disk_mounts, vec!["/mnt/data"]);
        assert_eq!(collector.probe_hosts, vec!["1.1.1.1"]);
        assert_eq!(collector.link_agent_process.as_deref(), Some("synorelayd"));
    }

    #[test]
    fn missing_overlay_defaults_visible() {
        let json =
            r#"{"schemaVersion":2,"sshAlias":"nas","nasHost":"192.0.2.10","navItems":[],"ui":{}}"#;
        let cfg: AppConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.overview_container_names.is_empty());
        assert!(cfg.ui.overlay.visible);
        assert!(!cfg.ui.autostart);
        assert_eq!(cfg.ui.overlay.display.precision, 1);
        assert!(cfg.ui.overlay.display.show_cpu_temp);
        assert!(!cfg.ui.overlay.display.show_mem_temp);
        assert!(cfg.ui.overlay.display.snap_hide);
        assert!(!cfg.ui.sidebar_collapsed);
        assert_eq!(cfg.ui.bloub_pin, "auto");
    }
}
