use crate::collector::{valid_ssh_alias, SharedSnapshot};
use crate::config::{self, AppConfig, OverlayDisplay, TitleRules};
use crate::desktop;
use crate::drives::{self, DriveMount};
use crate::model::{Candidate, HealthEvent, ItemHealth, NavItem};
use crate::nav;
use crate::search::{self, SearchResponse};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State, WebviewWindow};
#[cfg(not(windows))]
use tauri_plugin_autostart::ManagerExt;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub ssh_alias: Option<String>,
    pub nas_host: Option<String>,
    pub interval_ms: Option<u64>,
    pub internet_probe_enabled: Option<bool>,
    pub autostart: Option<bool>,
    pub sidebar_collapsed: Option<bool>,
    pub nav_compact: Option<bool>,
    pub bloub_pin: Option<String>,
    pub everything_http: Option<String>,
    pub alist_url: Option<String>,
    pub alist_container: Option<String>,
    pub alist_backend: Option<config::AlistBackend>,
    pub quark_mount: Option<String>,
    pub baidu_mount: Option<String>,
    pub everything_executable: Option<String>,
    pub net_iface: Option<String>,
    pub disk_mounts: Option<Vec<String>>,
    pub probe_hosts: Option<Vec<String>>,
    pub link_agent_process: Option<String>,
    pub watch_ports: Option<Vec<u16>>,
    pub container_titles: Option<TitleRules>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OverlayDisplayPatch {
    pub show_labels: Option<bool>,
    pub precision: Option<u8>,
    pub show_link: Option<bool>,
    pub show_internet: Option<bool>,
    pub show_cpu: Option<bool>,
    pub show_cpu_temp: Option<bool>,
    pub show_mem: Option<bool>,
    pub show_mem_temp: Option<bool>,
    pub show_net: Option<bool>,
    pub show_access: Option<bool>,
    pub show_ports: Option<bool>,
    pub snap_hide: Option<bool>,
    pub opaque: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayPatch {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub collapsed: Option<bool>,
    pub visible: Option<bool>,
    pub dock: Option<String>,
    pub display: Option<OverlayDisplayPatch>,
}

/// 自定义命令不受 capability 约束，悬浮窗只该读指标，写配置必须挡在主窗口。
fn require_main(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == "main" {
        return Ok(());
    }
    Err("该操作只能在主窗口执行".into())
}

fn require_main_or_pop(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == "main" || window.label() == "pop" {
        return Ok(());
    }
    Err("该操作只能在主窗口执行".into())
}

fn validate_search(cfg: &AppConfig) -> Result<(), String> {
    if !cfg.search.alist_url.trim().is_empty() {
        search::validate_alist_url(&cfg.search.alist_url)?;
        if !search::valid_container_name(&cfg.search.alist_container) {
            return Err("AList 容器名不合法".into());
        }
    }
    for path in [&cfg.search.quark_mount, &cfg.search.baidu_mount] {
        if !path.starts_with('/') || path.contains(['\n', '\r', '\0']) {
            return Err("网盘挂载路径必须是以 / 开头的绝对路径".into());
        }
    }
    if cfg.search.quark_mount.trim_end_matches('/') == cfg.search.baidu_mount.trim_end_matches('/')
    {
        return Err("两个网盘的挂载路径不能相同".into());
    }
    Ok(())
}

fn persist(state: &SharedSnapshot, cfg: AppConfig) -> Result<AppConfig, String> {
    validate_search(&cfg)?;
    config::save(&cfg).map_err(|e| e.to_string())?;
    let mut current = state.config.lock().map_err(|e| e.to_string())?;
    if search::connection_key(&current) != search::connection_key(&cfg) {
        search::invalidate_token();
    }
    *current = cfg.clone();
    Ok(cfg)
}

fn persist_and_emit(
    app: &AppHandle,
    state: &SharedSnapshot,
    cfg: AppConfig,
) -> Result<AppConfig, String> {
    let saved = persist(state, cfg)?;
    let _ = app.emit("config", &saved);
    Ok(saved)
}

fn sanitize_bloub_pin(value: &str) -> String {
    match value {
        "auto" | "idle" | "thinking" | "wink" | "wide" | "notify" | "alert" | "exclaim"
        | "sleep" | "orbit" => value.to_string(),
        _ => "auto".into(),
    }
}

fn apply_display_patch(display: &mut OverlayDisplay, patch: OverlayDisplayPatch) {
    if let Some(v) = patch.show_labels {
        display.show_labels = v;
    }
    if let Some(v) = patch.precision {
        display.precision = v.min(2);
    }
    if let Some(v) = patch.show_link {
        display.show_link = v;
    }
    if let Some(v) = patch.show_internet {
        display.show_internet = v;
    }
    if let Some(v) = patch.show_cpu {
        display.show_cpu = v;
    }
    if let Some(v) = patch.show_cpu_temp {
        display.show_cpu_temp = v;
    }
    if let Some(v) = patch.show_mem {
        display.show_mem = v;
    }
    if let Some(v) = patch.show_mem_temp {
        display.show_mem_temp = v;
    }
    if let Some(v) = patch.show_net {
        display.show_net = v;
    }
    if let Some(v) = patch.show_access {
        display.show_access = v;
    }
    if let Some(v) = patch.show_ports {
        display.show_ports = v;
    }
    if let Some(v) = patch.snap_hide {
        display.snap_hide = v;
    }
    if let Some(v) = patch.opaque {
        display.opaque = v;
    }
    display.clamp();
}

pub(crate) fn sync_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        let _ = app;
        return crate::autostart::sync(enabled);
    }
    #[cfg(not(windows))]
    {
        let mgr = app.autolaunch();
        if enabled {
            return mgr.enable().map_err(|e| format!("无法写入开机自启：{e}"));
        }
        match mgr.disable() {
            Ok(()) => Ok(()),
            Err(err) => match mgr.is_enabled() {
                Ok(false) => Ok(()),
                Ok(true) => Err(format!("无法关闭开机自启：{err}")),
                Err(_) => Ok(()),
            },
        }
    }
}

#[tauri::command]
pub fn autostart_status(app: AppHandle) -> Result<bool, String> {
    #[cfg(windows)]
    {
        let _ = app;
        crate::autostart::is_enabled()
    }
    #[cfg(not(windows))]
    {
        app.autolaunch().is_enabled().map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn get_config(state: State<Arc<SharedSnapshot>>) -> AppConfig {
    state.config()
}

#[tauri::command]
pub async fn fetch_nav_icon(url: String) -> Option<String> {
    crate::favicon::fetch_cached(&url).await
}

#[tauri::command]
pub fn export_config(
    window: WebviewWindow,
    state: State<Arc<SharedSnapshot>>,
) -> Result<String, String> {
    require_main(&window)?;
    serde_json::to_string_pretty(&state.config()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_config(
    app: AppHandle,
    window: WebviewWindow,
    state: State<Arc<SharedSnapshot>>,
    json: String,
) -> Result<AppConfig, String> {
    require_main(&window)?;
    let mut cfg: AppConfig =
        serde_json::from_str(&json).map_err(|e| format!("配置无法解析：{e}"))?;
    if cfg.schema_version > config::SCHEMA_VERSION {
        return Err("配置来自较新版本，请先升级应用".into());
    }
    cfg.schema_version = config::SCHEMA_VERSION;
    cfg.collector.sanitize();
    if cfg.ssh_alias.trim().is_empty() || cfg.nas_host.trim().is_empty() {
        return Err("SSH 别名和 NAS 地址不能为空".into());
    }
    if !valid_ssh_alias(cfg.ssh_alias.trim()) {
        return Err("SSH 别名只能是字母、数字、点、短横线和下划线，且不能以短横线开头".into());
    }
    for item in &mut cfg.nav_items {
        nav::apply_http_endpoint(item)?;
    }
    // Importing a backup does not opt this installation into login startup.
    cfg.ui.autostart = state.config().ui.autostart;
    let autostart = cfg.ui.autostart;
    search::invalidate_token();
    let saved = persist_and_emit(&app, &state, cfg)?;
    let _ = sync_autostart(&app, autostart);
    desktop::apply_overlay(&app, &saved);
    state.restart.notify_waiters();
    Ok(saved)
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    window: WebviewWindow,
    state: State<Arc<SharedSnapshot>>,
    patch: SettingsPatch,
) -> Result<AppConfig, String> {
    require_main(&window)?;
    let mut cfg = state.config();
    let mut reconnect = false;
    if let Some(alias) = patch.ssh_alias {
        let alias = alias.trim().to_string();
        if alias.is_empty() {
            return Err("SSH 别名不能为空".into());
        }
        if !valid_ssh_alias(&alias) {
            return Err("SSH 别名只能是字母、数字、点、短横线和下划线，且不能以短横线开头".into());
        }
        if alias != cfg.ssh_alias {
            reconnect = true;
        }
        cfg.ssh_alias = alias;
    }
    if let Some(host) = patch.nas_host {
        let host = host.trim().to_string();
        if host.is_empty() {
            return Err("NAS 地址不能为空".into());
        }
        cfg.nas_host = host;
    }
    if let Some(ms) = patch.interval_ms {
        if ![1000, 2000, 5000, 10000].contains(&ms) {
            return Err("刷新间隔只支持 1/2/5/10 秒".into());
        }
        if ms != cfg.ui.interval_ms {
            reconnect = true;
        }
        cfg.ui.interval_ms = ms;
    }
    if let Some(enabled) = patch.internet_probe_enabled {
        if enabled != cfg.ui.internet_probe_enabled {
            reconnect = true;
        }
        cfg.ui.internet_probe_enabled = enabled;
    }
    if let Some(enabled) = patch.autostart {
        cfg.ui.autostart = enabled;
        sync_autostart(&app, enabled)?;
    }
    if let Some(collapsed) = patch.sidebar_collapsed {
        cfg.ui.sidebar_collapsed = collapsed;
    }
    if let Some(compact) = patch.nav_compact {
        cfg.ui.nav_compact = compact;
    }
    if let Some(pin) = patch.bloub_pin {
        cfg.ui.bloub_pin = sanitize_bloub_pin(&pin);
    }
    if let Some(url) = patch.everything_http {
        let url = url.trim().to_string();
        if !nav::valid_http_url(&url) {
            return Err("Everything 地址无效".into());
        }
        cfg.search.everything_http = url;
    }
    if let Some(url) = patch.alist_url {
        let url = url.trim().trim_end_matches('/').to_string();
        if !url.is_empty() && !nav::valid_http_url(&url) {
            return Err("AList 地址无效".into());
        }
        cfg.search.alist_url = url;
    }
    if let Some(name) = patch.alist_container {
        let name = name.trim().to_string();
        if !search::valid_container_name(&name) {
            return Err("AList 容器名不合法".into());
        }
        cfg.search.alist_container = name;
    }
    if let Some(backend) = patch.alist_backend {
        cfg.search.alist_backend = backend;
    }
    if let Some(path) = patch.quark_mount {
        cfg.search.quark_mount = path.trim().to_string();
    }
    if let Some(path) = patch.baidu_mount {
        cfg.search.baidu_mount = path.trim().to_string();
    }
    if let Some(path) = patch.everything_executable {
        cfg.search.everything_executable = path.trim().to_string();
    }
    if patch.net_iface.is_some()
        || patch.disk_mounts.is_some()
        || patch.probe_hosts.is_some()
        || patch.link_agent_process.is_some()
    {
        reconnect = true;
    }
    if let Some(iface) = patch.net_iface {
        cfg.collector.net_iface = Some(iface);
    }
    if let Some(mounts) = patch.disk_mounts {
        cfg.collector.disk_mounts = mounts;
    }
    if let Some(hosts) = patch.probe_hosts {
        cfg.collector.probe_hosts = hosts;
    }
    if let Some(proc_name) = patch.link_agent_process {
        cfg.collector.link_agent_process = Some(proc_name);
    }
    cfg.collector.sanitize();
    if let Some(ports) = patch.watch_ports {
        cfg.watch_ports = ports;
    }
    if let Some(titles) = patch.container_titles {
        cfg.container_titles = titles;
    }
    let saved = persist_and_emit(&app, &state, cfg)?;
    if reconnect {
        state.restart.notify_waiters();
    }
    Ok(saved)
}

#[tauri::command]
pub fn list_candidates(state: State<Arc<SharedSnapshot>>) -> Vec<Candidate> {
    let cfg = state.config();
    let snap = state.get();
    let unmatched: Vec<(u16, u32)> = snap
        .active_access
        .iter()
        .filter(|a| a.nav_item_id.is_none())
        .map(|a| (a.port, a.connections))
        .collect();
    nav::candidates(
        &cfg.nas_host,
        &cfg.nav_items,
        &snap.containers,
        &unmatched,
        &cfg.container_titles,
    )
}

#[tauri::command]
pub fn add_nav_item(
    window: WebviewWindow,
    state: State<Arc<SharedSnapshot>>,
    candidate: Candidate,
) -> Result<AppConfig, String> {
    require_main(&window)?;
    let mut cfg = state.config();
    if cfg
        .nav_items
        .iter()
        .any(|i| i.host == candidate.host && i.port == candidate.port)
    {
        return Err("该端口已经收录".into());
    }
    let order = cfg.nav_items.iter().map(|i| i.order).max().unwrap_or(0) + 1;
    cfg.nav_items
        .push(nav::item_from_candidate(&candidate, order)?);
    persist(&state, cfg)
}

#[tauri::command]
pub fn update_nav_item(
    window: WebviewWindow,
    state: State<Arc<SharedSnapshot>>,
    item: NavItem,
) -> Result<AppConfig, String> {
    require_main(&window)?;
    if item.title.trim().is_empty() {
        return Err("名称不能为空".into());
    }
    let mut item = item;
    nav::apply_http_endpoint(&mut item)?;
    let mut cfg = state.config();
    let Some(slot) = cfg.nav_items.iter_mut().find(|i| i.id == item.id) else {
        return Err("找不到该导航项".into());
    };
    *slot = item;
    persist(&state, cfg)
}

#[tauri::command]
pub fn remove_nav_item(
    window: WebviewWindow,
    state: State<Arc<SharedSnapshot>>,
    id: String,
) -> Result<AppConfig, String> {
    require_main(&window)?;
    let mut cfg = state.config();
    cfg.nav_items.retain(|i| i.id != id);
    if let Ok(mut health) = state.health.lock() {
        health.remove(&id);
    }
    persist(&state, cfg)
}

#[tauri::command]
pub fn move_nav_item(
    window: WebviewWindow,
    state: State<Arc<SharedSnapshot>>,
    id: String,
    delta: i32,
) -> Result<AppConfig, String> {
    require_main(&window)?;
    let mut cfg = state.config();
    cfg.nav_items.sort_by_key(|i| i.order);
    let Some(index) = cfg.nav_items.iter().position(|i| i.id == id) else {
        return Err("找不到该导航项".into());
    };
    let next = index as i32 + delta;
    if next < 0 || next >= cfg.nav_items.len() as i32 {
        return Ok(cfg);
    }
    cfg.nav_items.swap(index, next as usize);
    for (order, item) in cfg.nav_items.iter_mut().enumerate() {
        item.order = order as u32 + 1;
    }
    persist(&state, cfg)
}

#[tauri::command]
pub fn set_overview_container(
    app: AppHandle,
    window: WebviewWindow,
    state: State<Arc<SharedSnapshot>>,
    name: String,
    visible: bool,
) -> Result<AppConfig, String> {
    require_main(&window)?;
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("容器名称不能为空".into());
    }
    let mut cfg = state.config();
    if visible {
        if !cfg.overview_container_names.iter().any(|n| n == &name) {
            cfg.overview_container_names.push(name);
        }
    } else {
        cfg.overview_container_names.retain(|n| n != &name);
    }
    persist_and_emit(&app, &state, cfg)
}

#[tauri::command]
pub fn set_overview_containers(
    app: AppHandle,
    window: WebviewWindow,
    state: State<Arc<SharedSnapshot>>,
    names: Vec<String>,
) -> Result<AppConfig, String> {
    require_main(&window)?;
    let mut cfg = state.config();
    cfg.replace_overview_containers(names);
    persist_and_emit(&app, &state, cfg)
}

#[tauri::command]
pub fn show_main_window(app: AppHandle) {
    desktop::show_main(&app);
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NavigatePayload {
    pub page: String,
    pub query: Option<String>,
}

#[derive(serde::Serialize, Clone)]
pub struct OverlayPopEvent {
    pub kind: String,
    pub token: u64,
}

fn pop_state() -> &'static std::sync::Mutex<OverlayPopEvent> {
    static POP: std::sync::OnceLock<std::sync::Mutex<OverlayPopEvent>> = std::sync::OnceLock::new();
    POP.get_or_init(|| {
        std::sync::Mutex::new(OverlayPopEvent {
            kind: "menu".into(),
            token: 0,
        })
    })
}

#[tauri::command]
pub fn open_main_page(app: AppHandle, page: String, query: Option<String>) {
    let page = match page.as_str() {
        "search" | "settings" | "nav" | "containers" => page,
        _ => "overview".into(),
    };
    desktop::show_main(&app);
    let _ = app.emit(
        "navigate",
        NavigatePayload {
            page,
            query: query.filter(|value| !value.trim().is_empty()),
        },
    );
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn open_overlay_pop(app: AppHandle, kind: String, x: i32, y: i32) -> Result<(), String> {
    let kind = if kind == "search" { "search" } else { "menu" };
    desktop::present_overlay_pop(&app, kind, x, y)?;
    let mut guard = pop_state().lock().map_err(|err| err.to_string())?;
    guard.token = guard.token.saturating_add(1);
    guard.kind = kind.into();
    let event = guard.clone();
    drop(guard);
    let _ = app.emit_to("pop", "overlay-pop", event);
    Ok(())
}

#[tauri::command]
pub fn overlay_pop_snapshot() -> OverlayPopEvent {
    pop_state()
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or(OverlayPopEvent {
            kind: "menu".into(),
            token: 0,
        })
}

#[tauri::command]
pub fn resize_overlay_pop(window: WebviewWindow, height: f64) -> Result<(), String> {
    desktop::resize_overlay_pop(&window, height)
}

#[tauri::command]
pub fn hide_overlay_pop(app: AppHandle) {
    desktop::hide_overlay_pop(&app);
}

#[tauri::command]
pub fn place_overlay(
    window: WebviewWindow,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<(), String> {
    if window.label() != "overlay" {
        return Err("只能移动悬浮窗".into());
    }
    desktop::set_overlay_outer_rect(&window, x, y, width, height);
    Ok(())
}

#[tauri::command]
pub fn set_overlay_clip(window: WebviewWindow, rect: Option<[i32; 4]>) -> Result<(), String> {
    if window.label() != "overlay" {
        return Err("只能裁切悬浮窗".into());
    }
    desktop::set_overlay_clip(&window, rect)
}

#[tauri::command]
pub fn set_overlay_state(
    app: AppHandle,
    state: State<Arc<SharedSnapshot>>,
    patch: OverlayPatch,
) -> Result<AppConfig, String> {
    let mut cfg = state.config();
    if patch.x.is_some() {
        cfg.ui.overlay.x = patch.x;
    }
    if patch.y.is_some() {
        cfg.ui.overlay.y = patch.y;
    }
    if let Some(collapsed) = patch.collapsed {
        cfg.ui.overlay.collapsed = collapsed;
    }
    if let Some(visible) = patch.visible {
        cfg.ui.overlay.visible = visible;
    }
    if let Some(dock) = patch.dock {
        cfg.ui.overlay.dock = if dock == "left" || dock == "right" {
            Some(dock)
        } else {
            None
        };
    }
    if let Some(display) = patch.display {
        apply_display_patch(&mut cfg.ui.overlay.display, display);
    }
    let saved = persist_and_emit(&app, &state, cfg)?;
    desktop::apply_overlay(&app, &saved);
    Ok(saved)
}

#[tauri::command]
pub fn open_access(state: State<Arc<SharedSnapshot>>, port: u16) -> Result<(), String> {
    if nav::skipped_port(port) {
        return Err("该端口不会在悬浮窗打开".into());
    }
    let cfg = state.config();
    let snap = state.get();
    let url = if let Some(item) = cfg.nav_items.iter().find(|item| item.port == port) {
        if !nav::valid_http_url(&item.url) {
            return Err("地址无效".into());
        }
        item.url.clone()
    } else {
        let known = snap.active_access.iter().any(|item| item.port == port)
            || snap
                .containers
                .iter()
                .any(|container| container.ports.iter().any(|map| map.host == port));
        if !known {
            return Err("未知端口".into());
        }
        nav::http_url(&cfg.nas_host, port)
    };
    tauri_plugin_opener::open_url(&url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_health(state: State<Arc<SharedSnapshot>>) -> HashMap<String, ItemHealth> {
    state.health.lock().map(|g| g.clone()).unwrap_or_default()
}

#[tauri::command]
pub fn get_events(state: State<Arc<SharedSnapshot>>) -> Vec<HealthEvent> {
    state.get_events()
}

#[tauri::command]
pub async fn run_search(
    window: WebviewWindow,
    state: State<'_, Arc<SharedSnapshot>>,
    query: String,
) -> Result<SearchResponse, String> {
    require_main_or_pop(&window)?;
    let cfg = state.config();
    search::run_search(&cfg, &query).await
}

#[tauri::command]
pub fn open_search_hit(window: WebviewWindow, path: String) -> Result<(), String> {
    require_main_or_pop(&window)?;
    search::open_local_path(&path)
}

#[tauri::command]
pub fn reveal_search_hit(window: WebviewWindow, path: String) -> Result<(), String> {
    require_main_or_pop(&window)?;
    search::reveal_local_path(&path)
}

#[tauri::command]
pub fn get_drive_mounts(state: State<Arc<SharedSnapshot>>) -> Vec<DriveMount> {
    state.get_drives()
}

#[tauri::command]
pub async fn refresh_drive_mounts(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, Arc<SharedSnapshot>>,
) -> Result<Vec<DriveMount>, String> {
    require_main(&window)?;
    let cfg = state.config();
    let mounts = drives::probe_drives(&cfg).await;
    state.store_drives(mounts.clone());
    let _ = app.emit("drives", &mounts);
    Ok(mounts)
}

#[tauri::command]
pub async fn remount_drive(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, Arc<SharedSnapshot>>,
    kind: String,
    credential: String,
) -> Result<Vec<DriveMount>, String> {
    require_main(&window)?;
    let cfg = state.config();
    let mounts = drives::remount_drive(&cfg, &kind, &credential).await?;
    state.store_drives(mounts.clone());
    let _ = app.emit("drives", &mounts);
    Ok(mounts)
}
