use crate::collector::SharedSnapshot;
use crate::config::AppConfig;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub fn overlay_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("overlay")
}

fn prepare_overlay_window(win: &WebviewWindow) {
    #[cfg(windows)]
    if let Err(err) = crate::overlay_frame::install(win) {
        eprintln!("overlay caption: {err}");
    }
    let _ = win.set_shadow(false);
    #[cfg(windows)]
    strip_overlay_frame(win);
    #[cfg(windows)]
    restore_overlay_transparency(win);
}

#[cfg(windows)]
fn strip_overlay_frame(win: &WebviewWindow) {
    let Ok(hwnd) = win.hwnd() else {
        return;
    };
    unsafe {
        const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
        const DWMWA_BORDER_COLOR: u32 = 34;
        const DWMWCP_DONOTROUND: u32 = 1;
        const DWMWA_COLOR_NONE: u32 = 0xFFFFFFFE;
        let hwnd = hwnd.0;
        let corner = DWMWCP_DONOTROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&corner as *const u32).cast(),
            4,
        );
        let color = DWMWA_COLOR_NONE;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR, (&color as *const u32).cast(), 4);
    }
}

#[cfg(windows)]
#[repr(C)]
struct DwmBlurBehind {
    dw_flags: u32,
    enable: i32,
    region: *mut core::ffi::c_void,
    transition_on_maximized: i32,
}

#[cfg(windows)]
#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: *mut core::ffi::c_void,
        attr: u32,
        value: *const core::ffi::c_void,
        size: u32,
    ) -> i32;
    fn DwmEnableBlurBehindWindow(hwnd: *mut core::ffi::c_void, bb: *const DwmBlurBehind) -> i32;
}

#[cfg(windows)]
#[link(name = "gdi32")]
extern "system" {
    fn CreateRectRgn(x1: i32, y1: i32, x2: i32, y2: i32) -> *mut core::ffi::c_void;
    fn DeleteObject(obj: *mut core::ffi::c_void) -> i32;
}

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn SetWindowRgn(
        hwnd: *mut core::ffi::c_void,
        region: *mut core::ffi::c_void,
        redraw: i32,
    ) -> i32;
    fn SetWindowPos(
        hwnd: *mut core::ffi::c_void,
        insert_after: *mut core::ffi::c_void,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
}

/// Limit the parked window to its visible capsule without resizing the WebView.
/// Clear this region before expanding so GSAP can use the full 520px surface.
pub fn set_overlay_clip(win: &WebviewWindow, rect: Option<[i32; 4]>) -> Result<(), String> {
    if let Some([left, top, right, bottom]) = rect {
        if left < 0 || top < 0 || right <= left || bottom <= top || right > 65536 || bottom > 65536
        {
            return Err("无效的悬浮窗区域".into());
        }
    }
    #[cfg(windows)]
    {
        let hwnd = win.hwnd().map_err(|e| e.to_string())?;
        unsafe {
            let region = match rect {
                Some([left, top, right, bottom]) => {
                    let region = CreateRectRgn(left, top, right, bottom);
                    if region.is_null() {
                        return Err("创建悬浮窗区域失败".into());
                    }
                    region
                }
                None => core::ptr::null_mut(),
            };
            if SetWindowRgn(hwnd.0, region, 1) == 0 {
                if !region.is_null() {
                    let _ = DeleteObject(region);
                }
                return Err("设置悬浮窗区域失败".into());
            }
            // On success Windows owns the region. Do not delete it here.
        }
    }
    Ok(())
}

#[cfg(windows)]
fn restore_overlay_transparency(win: &WebviewWindow) {
    let Ok(hwnd) = win.hwnd() else {
        return;
    };
    unsafe {
        const DWM_BB_ENABLE: u32 = 0x1;
        const DWM_BB_BLURREGION: u32 = 0x2;
        let region = CreateRectRgn(0, 0, -1, -1);
        let bb = DwmBlurBehind {
            dw_flags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
            enable: 1,
            region,
            transition_on_maximized: 0,
        };
        let _ = DwmEnableBlurBehindWindow(hwnd.0, &bb);
        if !region.is_null() {
            let _ = DeleteObject(region);
        }
    }
}

/// 一次写完位置和尺寸。贴右边变宽时，Tauri 的 setSize + setPosition 会中间露出一帧错原点。
pub fn set_overlay_outer_rect(win: &WebviewWindow, x: i32, y: i32, width: i32, height: i32) {
    let width = width.max(1);
    let height = height.max(1);
    #[cfg(windows)]
    if let Ok(hwnd) = win.hwnd() {
        unsafe {
            const SWP_NOZORDER: u32 = 0x0004;
            const SWP_NOACTIVATE: u32 = 0x0010;
            let _ = SetWindowPos(
                hwnd.0,
                core::ptr::null_mut(),
                x,
                y,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        restore_overlay_transparency(win);
        return;
    }
    let _ = win.set_size(PhysicalSize::new(width as u32, height as u32));
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    let _ = app.emit("main-shown", ());
}

fn prepare_pop_window(win: &WebviewWindow) {
    let _ = win.set_shadow(false);
    #[cfg(windows)]
    strip_overlay_frame(win);
    #[cfg(windows)]
    restore_overlay_transparency(win);
}

fn pop_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window("pop")
        .ok_or_else(|| "弹出层窗口未初始化".to_string())
}

fn monitor_of(win: &WebviewWindow, x: i32, y: i32) -> Option<tauri::Monitor> {
    win.monitor_from_point(x as f64, y as f64)
        .ok()
        .flatten()
        .or_else(|| win.current_monitor().ok().flatten())
        .or_else(|| win.primary_monitor().ok().flatten())
}

/// `kind` is `menu` or `search`. Anchor is a physical screen point.
pub fn present_overlay_pop(
    app: &AppHandle,
    kind: &str,
    anchor_x: i32,
    anchor_y: i32,
) -> Result<(), String> {
    let win = pop_window(app)?;
    let search = kind == "search";
    let logical = if search {
        (616.0, 176.0)
    } else {
        (236.0, 220.0)
    };
    let scale = monitor_of(&win, anchor_x, anchor_y)
        .as_ref()
        .map(|monitor| monitor.scale_factor())
        .unwrap_or(1.0);
    let width = (logical.0 * scale).round().max(1.0) as i32;
    let height = (logical.1 * scale).round().max(1.0) as i32;
    let menu_x = anchor_x - (12.0 * scale).round() as i32;
    let menu_y = anchor_y + (10.0 * scale).round() as i32;
    let (x, y) = if let Some(monitor) = monitor_of(&win, anchor_x, anchor_y) {
        let mx = monitor.position().x;
        let my = monitor.position().y;
        let mw = monitor.size().width as i32;
        let mh = monitor.size().height as i32;
        let max_x = (mx + mw - width - 8).max(mx + 8);
        let max_y = (my + mh - height - 8).max(my + 8);
        if search {
            let x = mx + (mw - width) / 2;
            let y = my + (mh as f64 * 0.14) as i32;
            (x.clamp(mx + 8, max_x), y.clamp(my + 8, max_y))
        } else {
            (menu_x.clamp(mx + 8, max_x), menu_y.clamp(my + 8, max_y))
        }
    } else {
        (menu_x, menu_y)
    };
    #[cfg(windows)]
    if let Ok(hwnd) = win.hwnd() {
        unsafe {
            const SWP_SHOWWINDOW: u32 = 0x0040;
            const HWND_TOPMOST: isize = -1;
            let _ = SetWindowPos(
                hwnd.0,
                HWND_TOPMOST as *mut core::ffi::c_void,
                x,
                y,
                width,
                height,
                SWP_SHOWWINDOW,
            );
        }
    }
    #[cfg(not(windows))]
    {
        let _ = win.set_size(PhysicalSize::new(width as u32, height as u32));
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }
    prepare_pop_window(&win);
    let _ = win.show();
    let _ = win.set_focus();
    let _ = win.set_always_on_top(true);
    Ok(())
}

pub fn resize_overlay_pop(win: &WebviewWindow, height_logical: f64) -> Result<(), String> {
    if win.label() != "pop" {
        return Err("只能调整弹出层".into());
    }
    let pos = win.outer_position().map_err(|err| err.to_string())?;
    let scale = monitor_of(win, pos.x, pos.y)
        .as_ref()
        .map(|monitor| monitor.scale_factor())
        .unwrap_or(1.0);
    let width = (616.0 * scale).round().max(1.0) as u32;
    let height = (height_logical.clamp(96.0, 560.0) * scale).round().max(1.0) as u32;
    let _ = win.set_size(PhysicalSize::new(width, height));
    prepare_pop_window(win);
    Ok(())
}

pub fn hide_overlay_pop(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("pop") {
        let _ = win.hide();
    }
}

const OVERLAY_EDGE_PAD: f64 = 10.0;

fn clamp_overlay_position(win: &WebviewWindow, x: i32, y: i32) -> PhysicalPosition<i32> {
    let size = win.outer_size().unwrap_or(PhysicalSize::new(80, 80));
    let monitor = win
        .monitor_from_point(x as f64, y as f64)
        .ok()
        .flatten()
        .or_else(|| {
            win.monitor_from_point(
                x as f64 + f64::from(size.width) / 2.0,
                y as f64 + f64::from(size.height) / 2.0,
            )
            .ok()
            .flatten()
        })
        .or_else(|| win.current_monitor().ok().flatten())
        .or_else(|| win.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return PhysicalPosition::new(x.max(8), y.max(8));
    };
    let mx = monitor.position().x;
    let my = monitor.position().y;
    let mw = monitor.size().width as i32;
    let mh = monitor.size().height as i32;
    let pad = (OVERLAY_EDGE_PAD * monitor.scale_factor()).round() as i32;
    let min_x = mx + pad;
    let max_x = (mx + mw - size.width as i32 - pad).max(min_x);
    let min_y = my + pad;
    let max_y = (my + mh - size.height as i32 - pad).max(min_y);
    PhysicalPosition::new(x.clamp(min_x, max_x), y.clamp(min_y, max_y))
}

pub fn apply_overlay(app: &AppHandle, cfg: &AppConfig) {
    let Some(win) = overlay_window(app) else {
        return;
    };
    prepare_overlay_window(&win);
    if let (Some(x), Some(y)) = (cfg.ui.overlay.x, cfg.ui.overlay.y) {
        let target = clamp_overlay_position(&win, x, y);
        let already = win
            .outer_position()
            .ok()
            .is_some_and(|pos| pos.x == target.x && pos.y == target.y);
        if !already {
            let _ = win.set_position(target);
        }
    } else if let Ok(Some(monitor)) = win.primary_monitor().or_else(|_| win.current_monitor()) {
        let scale = monitor.scale_factor();
        let size = monitor.size();
        let x = monitor.position().x + size.width as i32 - (530.0 * scale) as i32;
        let y = monitor.position().y + (20.0 * scale) as i32;
        let _ = win.set_position(clamp_overlay_position(&win, x, y));
    }
    if cfg.ui.overlay.visible {
        let _ = win.show();
    } else {
        let _ = win.hide();
    }
}

pub fn persist_overlay_visible(app: &AppHandle, visible: bool) {
    if let Some(state) = app.try_state::<Arc<SharedSnapshot>>() {
        let mut cfg = state.config();
        cfg.ui.overlay.visible = visible;
        let _ = crate::config::save(&cfg);
        if let Ok(mut guard) = state.config.lock() {
            *guard = cfg.clone();
        }
        let _ = app.emit("config", &cfg);
    }
}

pub fn toggle_overlay(app: &AppHandle) {
    let Some(win) = overlay_window(app) else {
        return;
    };
    let visible = win.is_visible().unwrap_or(false);
    if visible {
        let _ = win.hide();
    } else {
        let _ = win.show();
    }
    persist_overlay_visible(app, !visible);
}

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle_overlay", "显示/隐藏悬浮窗", true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open_main", "打开主窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &open, &quit])?;
    TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().expect("app icon"))
        .tooltip("NAS Lighthouse")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle_overlay" => toggle_overlay(app),
            "open_main" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn register_shortcut(app: &AppHandle) -> Result<(), String> {
    let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyN);
    let manager = app.global_shortcut();
    if manager.is_registered(shortcut) {
        return Ok(());
    }
    manager
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                toggle_overlay(app);
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod overlay_live_tests {
    /// Keep in sync with `src/lib/overlayLive.ts`.
    fn overlay_bloub_live(shown: bool, collapsed: bool, peeked: bool, hovered: bool) -> bool {
        shown && (!collapsed || peeked || hovered)
    }

    fn bloub_loop_active(live: bool, reduced_motion: bool) -> bool {
        live && !reduced_motion
    }

    #[test]
    fn expanded_bar_keeps_animating() {
        assert!(overlay_bloub_live(true, false, false, false));
        assert!(bloub_loop_active(true, false));
    }

    #[test]
    fn parked_dot_is_still() {
        assert!(!overlay_bloub_live(true, true, false, false));
        assert!(!bloub_loop_active(false, false));
    }

    #[test]
    fn hover_or_peek_wakes_parked_dot() {
        assert!(overlay_bloub_live(true, true, false, true));
        assert!(overlay_bloub_live(true, true, true, false));
    }

    #[test]
    fn hidden_overlay_never_animates() {
        assert!(!overlay_bloub_live(false, false, true, true));
        assert!(!bloub_loop_active(true, true));
    }
}
