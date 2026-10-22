#[cfg(windows)]
mod autostart;
mod collector;
mod commands;
mod config;
mod desktop;
mod drives;
mod events;
mod favicon;
mod health;
mod local_conn;
mod model;
mod nav;
#[cfg(windows)]
mod overlay_frame;
mod parse;
mod probe;
mod search;
mod state;

use collector::SharedSnapshot;
use events::{EventSignal, SignalLevel};
use health::unknown;
use model::{HealthState, ItemHealth, MetricsSnapshot};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, Manager};

#[tauri::command]
fn get_snapshot(shared: tauri::State<Arc<SharedSnapshot>>) -> MetricsSnapshot {
    shared.get()
}

fn main_is_visible(app: &tauri::AppHandle) -> bool {
    let Some(window) = app.get_webview_window("main") else {
        return true;
    };
    let visible = window.is_visible().unwrap_or(true);
    let minimized = window.is_minimized().unwrap_or(false);
    visible && !minimized
}

async fn run_health_loop(app: tauri::AppHandle, state: Arc<SharedSnapshot>) {
    let mut first = true;
    loop {
        let visible = main_is_visible(&app);
        tokio::time::sleep(Duration::from_secs(if first {
            2
        } else if visible {
            30
        } else {
            180
        }))
        .await;
        first = false;
        let items: Vec<_> = state
            .config()
            .nav_items
            .into_iter()
            .filter(|item| item.health_check)
            .collect();
        if items.is_empty() {
            continue;
        }
        let mut set = tokio::task::JoinSet::new();
        let limit = Arc::new(tokio::sync::Semaphore::new(4));
        for item in items {
            let limit = limit.clone();
            set.spawn(async move {
                let Ok(_permit) = limit.acquire_owned().await else {
                    return (item.id, false, None, Some("探测任务已取消".into()));
                };
                let (ok, ms, reason) =
                    health::tcp_alive(&item.host, item.port, Duration::from_secs(2)).await;
                (item.id, ok, ms, reason)
            });
        }
        while let Some(joined) = set.join_next().await {
            let Ok((id, ok, ms, reason)) = joined else {
                continue;
            };
            let prev = state
                .health
                .lock()
                .ok()
                .and_then(|g| g.get(&id).cloned())
                .unwrap_or_else(unknown);
            let streak = state
                .fail_streak
                .lock()
                .ok()
                .and_then(|g| g.get(&id).copied())
                .unwrap_or(0);
            let (next, next_streak) = health::apply_probe(&prev, streak, ok, ms, reason);
            if let Ok(mut g) = state.health.lock() {
                g.insert(id.clone(), next);
            }
            if let Ok(mut g) = state.fail_streak.lock() {
                g.insert(id, next_streak);
            }
        }
        let map: HashMap<String, ItemHealth> =
            state.health.lock().map(|g| g.clone()).unwrap_or_default();
        let _ = app.emit("health", &map);
    }
}

fn event_signals(state: &SharedSnapshot) -> Vec<EventSignal> {
    let snap = state.get();
    let cfg = state.config();
    let health = state.health.lock().map(|g| g.clone()).unwrap_or_default();
    let drives = state.get_drives();
    let mut out = Vec::new();

    let (link_level, link_message) = match snap.link {
        model::LinkState::Connected => (SignalLevel::Normal, "采集流正常"),
        model::LinkState::Reconnecting if snap.last_success_ts.is_some() => {
            (SignalLevel::Warning, "正在重新连接 NAS")
        }
        model::LinkState::Reconnecting => (SignalLevel::Normal, "正在建立首次连接"),
        model::LinkState::Disconnected => (SignalLevel::Critical, "本机与 NAS 已断联"),
    };
    out.push(EventSignal {
        source: "nas".into(),
        source_id: "collector".into(),
        title: "NAS 采集".into(),
        level: link_level,
        message: link_message.into(),
        reason: None,
    });

    out.push(EventSignal {
        source: "internet".into(),
        source_id: "nas-outbound".into(),
        title: "NAS 出站网络".into(),
        level: if snap.internet.state == model::InternetState::Down {
            SignalLevel::Critical
        } else {
            SignalLevel::Normal
        },
        message: if snap.internet.state == model::InternetState::Down {
            "无法连接互联网".into()
        } else {
            "出站网络正常".into()
        },
        reason: None,
    });

    for raid in &snap.raids {
        let (level, message) = if raid.recovering {
            (
                SignalLevel::Warning,
                format!(
                    "阵列正在重建 {}%",
                    raid.recovery_pct
                        .map(|v| format!("{v:.1}"))
                        .unwrap_or_else(|| "—".into())
                ),
            )
        } else if !raid.healthy {
            (SignalLevel::Critical, "阵列状态异常".into())
        } else {
            (SignalLevel::Normal, "阵列健康".into())
        };
        out.push(EventSignal {
            source: "raid".into(),
            source_id: raid.name.clone(),
            title: format!("阵列 {}", raid.name),
            level,
            message,
            reason: if raid.healthy {
                None
            } else {
                Some(format!("{}/{} 块磁盘在线", raid.disks_up, raid.disks_total))
            },
        });
    }

    for item in &cfg.nav_items {
        let Some(item_health) = health.get(&item.id) else {
            continue;
        };
        let (level, message) = match item_health.state {
            HealthState::Unknown => (SignalLevel::Normal, "尚未探测"),
            HealthState::Up => (SignalLevel::Normal, "服务在线"),
            HealthState::Degraded => (SignalLevel::Warning, "首次探测失败"),
            HealthState::Down => (SignalLevel::Critical, "连续探测失败"),
        };
        out.push(EventSignal {
            source: "service".into(),
            source_id: item.id.clone(),
            title: item.title.clone(),
            level,
            message: message.into(),
            reason: item_health.reason.clone(),
        });
    }

    for name in &cfg.overview_container_names {
        let Some(container) = snap
            .containers
            .iter()
            .find(|container| &container.name == name)
        else {
            if !snap.containers.is_empty() {
                out.push(EventSignal {
                    source: "container".into(),
                    source_id: name.clone(),
                    title: name.clone(),
                    level: SignalLevel::Critical,
                    message: "已关注容器未在运行列表中".into(),
                    reason: Some("容器可能已停止或被删除".into()),
                });
            }
            continue;
        };
        let unhealthy = container
            .health
            .as_deref()
            .map(|value| value.eq_ignore_ascii_case("unhealthy"))
            .unwrap_or(false);
        out.push(EventSignal {
            source: "container".into(),
            source_id: name.clone(),
            title: name.clone(),
            level: if unhealthy {
                SignalLevel::Critical
            } else {
                SignalLevel::Normal
            },
            message: if unhealthy {
                "容器健康检查失败".into()
            } else {
                "容器运行正常".into()
            },
            reason: unhealthy.then(|| container.status.clone()),
        });
    }

    for drive in drives {
        let level = match drive.state.as_str() {
            "down" => SignalLevel::Critical,
            "missing" => SignalLevel::Warning,
            _ => SignalLevel::Normal,
        };
        out.push(EventSignal {
            source: "drive".into(),
            source_id: drive.id,
            title: format!("{}网盘", drive.label),
            level,
            message: drive.message,
            reason: None,
        });
    }

    out
}

async fn run_event_loop(app: tauri::AppHandle, state: Arc<SharedSnapshot>) {
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        if let Some(events) = state.observe_events(event_signals(&state)) {
            let _ = app.emit("events", events);
        }
    }
}

async fn run_access_loop(app: tauri::AppHandle, state: Arc<SharedSnapshot>) {
    let mut dwell = local_conn::AccessDwell::default();
    loop {
        let cfg = state.config();
        let nas_host = cfg.nas_host.clone();
        let ips = nav::resolve_host_ips(&nas_host).await;
        if !ips.is_empty() {
            let snap = state.get();
            let extra_ports: Vec<u16> = cfg
                .nav_items
                .iter()
                .map(|item| item.port)
                .chain(
                    snap.containers
                        .iter()
                        .flat_map(|c| c.ports.iter().map(|p| p.host)),
                )
                .chain(cfg.watch_ports.iter().copied())
                .collect();
            let conns = local_conn::collect_established(&ips, &extra_ports);
            let conns = dwell.observe(conns);
            let access = local_conn::match_access(
                &conns,
                &ips,
                &cfg.nav_items,
                &snap.containers,
                &nas_host,
                &cfg.watch_ports,
                &cfg.container_titles,
            );
            state.store_access(access);
            let _ = app.emit("metrics", &state.get());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn run_drive_loop(app: tauri::AppHandle, state: Arc<SharedSnapshot>) {
    let mut first = true;
    loop {
        tokio::time::sleep(Duration::from_secs(if first { 3 } else { 3600 })).await;
        first = false;
        if !main_is_visible(&app) {
            continue;
        }
        let cfg = state.config();
        let mounts = crate::drives::probe_drives(&cfg).await;
        state.store_drives(mounts.clone());
        let _ = app.emit("drives", &mounts);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(windows)]
    if cfg!(debug_assertions) {
        autostart::abort_if_dev_frontend_missing();
    }
    let config = config::load().unwrap_or_default();
    let shared = SharedSnapshot::new(config);
    let shared_setup = shared.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            desktop::show_main(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("nas-lighthouse")
                .build(),
        )
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(shared)
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            commands::get_config,
            commands::export_config,
            commands::import_config,
            commands::save_settings,
            commands::autostart_status,
            commands::list_candidates,
            commands::add_nav_item,
            commands::update_nav_item,
            commands::remove_nav_item,
            commands::move_nav_item,
            commands::set_overview_container,
            commands::set_overview_containers,
            commands::show_main_window,
            commands::open_main_page,
            commands::quit_app,
            commands::open_overlay_pop,
            commands::overlay_pop_snapshot,
            commands::resize_overlay_pop,
            commands::hide_overlay_pop,
            commands::place_overlay,
            commands::set_overlay_clip,
            commands::set_overlay_state,
            commands::open_access,
            commands::get_health,
            commands::get_events,
            commands::fetch_nav_icon,
            commands::run_search,
            commands::open_search_hit,
            commands::reveal_search_hit,
            commands::get_drive_mounts,
            commands::refresh_drive_mounts,
            commands::remount_drive
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main"
                    || window.label() == "overlay"
                    || window.label() == "pop"
                {
                    api.prevent_close();
                    let _ = window.hide();
                    if window.label() == "overlay" {
                        desktop::persist_overlay_visible(&window.app_handle(), false);
                    }
                }
            }
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            let health_handle = handle.clone();
            let access_handle = handle.clone();
            let drive_handle = handle.clone();
            let event_handle = handle.clone();
            let collector_state = shared_setup.clone();
            let health_state = shared_setup.clone();
            let access_state = shared_setup.clone();
            let drive_state = shared_setup;
            let event_state = collector_state.clone();
            let cfg = collector_state.config();
            desktop::apply_overlay(app.handle(), &cfg);
            // 只在配置明确要求自启时刷新 Run 项。配置没读到或默认关闭时不要
            // 去删注册表：开机启动 cwd/环境异常时会把刚写入的启动项清掉。
            if cfg.ui.autostart {
                if let Err(err) = commands::sync_autostart(app.handle(), true) {
                    eprintln!("autostart: {err}");
                }
            }
            if let Err(err) = desktop::setup_tray(app.handle()) {
                eprintln!("tray: {err}");
            }
            if let Err(err) = desktop::register_shortcut(app.handle()) {
                eprintln!("shortcut Ctrl+Alt+N: {err}");
            }
            tauri::async_runtime::spawn(async move {
                collector::run_forever(collector_state, move |snap| {
                    let _ = handle.emit("metrics", &snap);
                })
                .await;
            });
            tauri::async_runtime::spawn(async move {
                run_health_loop(health_handle, health_state).await;
            });
            tauri::async_runtime::spawn(async move {
                run_access_loop(access_handle, access_state).await;
            });
            tauri::async_runtime::spawn(async move {
                run_drive_loop(drive_handle, drive_state).await;
            });
            tauri::async_runtime::spawn(async move {
                run_event_loop(event_handle, event_state).await;
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

pub use probe::run as run_probe;
