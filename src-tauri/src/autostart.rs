//! Windows 开机自启：直接写 HKCU Run，不走 auto-launch。
//!
//! auto-launch / tauri-plugin-autostart 在 Windows 上有两处会让开关“看起来失败”：
//! 1. Run 值不给路径加引号，带空格的安装目录开机后根本拉不起来。
//! 2. `is_enabled` 把 StartupApproved 末 8 字节全 0 当成“已启用”。任务管理器或
//!    设置页碰过该项后会写入 FILETIME，于是 0x06（实际已启用）被判成未登记。
//! 状态只看首字节：0x02 / 0x06 / 0x08 为启用，0x03 等为禁用。

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use winreg::enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, KEY_WRITE, REG_BINARY};
use winreg::{RegKey, RegValue};

const RUN_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_KEY: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const VALUE_NAME: &str = "nas-lighthouse";
/// Only this application's install directories; other installations stay independent.
const INSTALL_DIRS: &[&str] = &["NAS Lighthouse", "nas-lighthouse"];
const ENABLED_BYTES: [u8; 12] = [0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

pub fn sync(enabled: bool) -> Result<(), String> {
    if enabled {
        enable()?;
        if !is_enabled()? {
            return Err(
                "启动项已写入，但系统仍标记为禁用。请在任务管理器「启动应用」里打开 NAS Lighthouse。"
                    .into(),
            );
        }
        return Ok(());
    }
    disable()
}

pub fn enable() -> Result<(), String> {
    let command = match startup_command() {
        Ok(command) => command,
        Err(err) => {
            let _ = disable();
            return Err(err);
        }
    };
    write_run(VALUE_NAME, &command)?;
    write_approved_enabled(VALUE_NAME)?;
    Ok(())
}

pub fn disable() -> Result<(), String> {
    delete_named(VALUE_NAME)?;
    Ok(())
}

pub fn is_enabled() -> Result<bool, String> {
    is_enabled_named(VALUE_NAME)
}

/// 开发版 WebView 会去连 localhost:1420。从开机启动或资源管理器直接打开时 Vite 不在，
/// 用户只会看到「无法连接」。这种启动直接退出，避免再登记 debug exe。
pub fn abort_if_dev_frontend_missing() {
    if std::env::var_os("TAURI_ENV_DEBUG").is_some()
        || std::env::var_os("TAURI_ENV_PLATFORM").is_some()
    {
        return;
    }
    let vite_up = dev_frontend_reachable(1420);
    if vite_up {
        return;
    }
    native_error(
        "这是开发调试版，开机后没有 Vite，所以会提示 localhost 无法连接。\n\n请用打包后的 NAS Lighthouse 做开机自启，不要启动 target\\debug 里的程序。",
    );
    std::process::exit(1);
}

fn dev_frontend_reachable(port: u16) -> bool {
    // Vite may bind localhost to either loopback family. Probing only
    // 127.0.0.1 rejects a healthy server that is listening on ::1.
    let addrs = [
        std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, port)),
        std::net::SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, port)),
    ];
    addrs.into_iter().any(|addr| {
        std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(300)).is_ok()
    })
}

#[cfg(windows)]
fn native_error(message: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(
            hwnd: *mut core::ffi::c_void,
            text: *const u16,
            caption: *const u16,
            flags: u32,
        ) -> i32;
    }
    fn wide(text: &str) -> Vec<u16> {
        OsStr::new(text).encode_wide().chain(Some(0)).collect()
    }
    let text = wide(message);
    let caption = wide("NAS Lighthouse");
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            0x0000_0010,
        );
    }
}

pub(crate) fn quote_exe(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let stripped = strip_extended_prefix(raw.as_ref());
    format!("\"{}\"", stripped.trim_matches('"'))
}

pub(crate) fn strip_extended_prefix(path: &str) -> &str {
    path.strip_prefix(r"\\?\").unwrap_or(path)
}

/// StartupApproved 首字节：02/06/08 启用，其它视为禁用。缺项等于未拦过，算启用。
pub(crate) fn startup_approved_disabled(bytes: &[u8]) -> bool {
    match bytes.first() {
        Some(0x02 | 0x06 | 0x08) | None => false,
        Some(_) => true,
    }
}

pub(crate) fn is_debug_run_command(command: &str) -> bool {
    let lower = command.replace('/', r"\").to_ascii_lowercase();
    lower.contains(r"\target\debug\")
}

fn is_enabled_named(name: &str) -> Result<bool, String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu
        .open_subkey_with_flags(RUN_KEY, KEY_QUERY_VALUE)
        .map_err(|e| format!("无法读取启动项：{e}"))?;
    let command = run.get_value::<String, _>(name).unwrap_or_default();
    if command.is_empty() || is_debug_run_command(&command) {
        return Ok(false);
    }
    let disabled = hkcu
        .open_subkey_with_flags(APPROVED_KEY, KEY_QUERY_VALUE)
        .ok()
        .and_then(|key| key.get_raw_value(name).ok())
        .is_some_and(|value| startup_approved_disabled(&value.bytes));
    Ok(!disabled)
}

fn write_run(name: &str, command: &str) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey_with_flags(RUN_KEY, KEY_WRITE | KEY_QUERY_VALUE)
        .map_err(|e| format!("无法打开启动项：{e}"))?;
    key.set_value(name, &command)
        .map_err(|e| format!("无法写入启动项：{e}"))
}

fn write_approved_enabled(name: &str) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey_with_flags(APPROVED_KEY, KEY_WRITE | KEY_QUERY_VALUE)
        .map_err(|e| format!("无法打开启动项状态：{e}"))?;
    key.set_raw_value(
        name,
        &RegValue {
            bytes: ENABLED_BYTES.to_vec(),
            vtype: REG_BINARY,
        },
    )
    .map_err(|e| format!("无法启用启动项：{e}"))
}

fn delete_named(name: &str) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE) {
        ignore_missing(key.delete_value(name)).map_err(|e| format!("无法删除启动项：{e}"))?;
    }
    if let Ok(key) = hkcu.open_subkey_with_flags(APPROVED_KEY, KEY_SET_VALUE) {
        let _ = ignore_missing(key.delete_value(name));
    }
    Ok(())
}

fn ignore_missing(result: std::io::Result<()>) -> std::io::Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) if err.raw_os_error() == Some(2) => Ok(()),
        Err(err) => Err(err),
    }
}

pub(crate) fn is_debug_target_exe(path: &Path) -> bool {
    let parent = path.parent();
    let debug = parent
        .and_then(|p| p.file_name())
        .is_some_and(|name| name.eq_ignore_ascii_case("debug"));
    let target = parent
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .is_some_and(|name| name.eq_ignore_ascii_case("target"));
    debug && target
}

fn installed_or_release_exe() -> Option<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        dirs.extend(
            INSTALL_DIRS
                .iter()
                .map(|name| PathBuf::from(&local).join(name)),
        );
    }
    if let Ok(pf) = std::env::var("ProgramFiles") {
        dirs.extend(
            INSTALL_DIRS
                .iter()
                .map(|name| PathBuf::from(&pf).join(name)),
        );
    }
    if let Ok(current) = std::env::current_exe() {
        if is_debug_target_exe(&current) {
            if let Some(target) = current.parent().and_then(|p| p.parent()) {
                dirs.push(target.join("release"));
            }
        }
    }
    const NAMES: &[&str] = &["nas-lighthouse.exe"];
    for dir in dirs {
        for name in NAMES {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn resolve_startup_exe() -> Result<PathBuf, String> {
    let current = std::env::current_exe().map_err(|e| format!("无法定位当前程序：{e}"))?;
    if !is_debug_target_exe(&current) {
        return Ok(current);
    }
    if let Some(found) = installed_or_release_exe() {
        return Ok(found);
    }
    Err(
        "当前是开发调试版，开机后没有 Vite 开发服务，窗口会是空的。请先打包安装（npm run tauri build），再用安装后的 NAS Lighthouse 打开开机自启。"
            .into(),
    )
}

pub(crate) fn startup_command() -> Result<String, String> {
    Ok(format!(
        "{} --autostart",
        quote_exe(&resolve_startup_exe()?)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn dev_frontend_probe_accepts_ipv6_loopback() {
        let Ok(listener) = std::net::TcpListener::bind((std::net::Ipv6Addr::LOCALHOST, 0)) else {
            return;
        };
        let port = listener.local_addr().expect("listener address").port();
        assert!(dev_frontend_reachable(port));
    }

    #[test]
    fn quotes_paths_with_spaces_and_strips_extended_prefix() {
        let path = PathBuf::from(r"\\?\C:\Program Files\NAS Lighthouse\nas-lighthouse.exe");
        assert_eq!(
            quote_exe(&path),
            r#""C:\Program Files\NAS Lighthouse\nas-lighthouse.exe""#
        );
    }

    #[test]
    fn quotes_paths_without_spaces() {
        let path =
            PathBuf::from(r"C:\Users\Example\AppData\Local\nas-lighthouse\nas-lighthouse.exe");
        assert_eq!(
            quote_exe(&path),
            r#""C:\Users\Example\AppData\Local\nas-lighthouse\nas-lighthouse.exe""#
        );
    }

    #[test]
    fn debug_target_exe_is_not_a_login_item() {
        assert!(is_debug_target_exe(Path::new(
            r"C:\src\nas-lighthouse\src-tauri\target\debug\nas-lighthouse.exe"
        )));
        assert!(!is_debug_target_exe(Path::new(
            r"C:\src\nas-lighthouse\src-tauri\target\release\nas-lighthouse.exe"
        )));
        assert!(!is_debug_target_exe(Path::new(
            r"C:\Users\Example\AppData\Local\nas-lighthouse\nas-lighthouse.exe"
        )));
        assert!(is_debug_run_command(
            r#""C:\src\nas-lighthouse\src-tauri\target\debug\nas-lighthouse.exe""#
        ));
        assert!(!is_debug_run_command(
            r#""C:\Users\Example\AppData\Local\nas-lighthouse\nas-lighthouse.exe" --autostart"#
        ));
    }

    #[test]
    fn approved_02_and_06_count_as_enabled() {
        assert!(!startup_approved_disabled(&[
            0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ]));
        assert!(!startup_approved_disabled(&[
            0x06, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8
        ]));
        assert!(!startup_approved_disabled(&[0x08, 0, 0, 0]));
        assert!(!startup_approved_disabled(&[]));
    }

    #[test]
    fn approved_03_counts_as_disabled() {
        assert!(startup_approved_disabled(&[
            0x03, 0, 0, 0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88
        ]));
        assert!(startup_approved_disabled(&[0x01, 0, 0, 0]));
    }

    #[test]
    #[ignore = "writes HKCU test entries; run explicitly on an isolated Windows runner"]
    fn registry_roundtrip_survives_startup_approved_timestamp() {
        const NAME: &str = "nas-lighthouse-selftest";
        struct Guard;
        impl Drop for Guard {
            fn drop(&mut self) {
                let _ = delete_named(NAME);
            }
        }
        let _guard = Guard;

        write_run(NAME, r#""C:\tmp\nas-lighthouse-selftest.exe""#).expect("write run");
        write_approved_enabled(NAME).expect("write approved");
        assert!(is_enabled_named(NAME).expect("enabled after 02"));

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (approved, _) = hkcu
            .create_subkey_with_flags(APPROVED_KEY, KEY_WRITE | KEY_QUERY_VALUE)
            .expect("open approved");
        approved
            .set_raw_value(
                NAME,
                &RegValue {
                    bytes: vec![0x06, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8],
                    vtype: REG_BINARY,
                },
            )
            .expect("stamp 06");
        assert!(
            is_enabled_named(NAME).expect("06 with FILETIME must stay enabled"),
            "auto-launch 会把带时间戳的 06 判成未启用，这里必须判成已启用"
        );

        approved
            .set_raw_value(
                NAME,
                &RegValue {
                    bytes: vec![0x03, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8],
                    vtype: REG_BINARY,
                },
            )
            .expect("stamp 03");
        assert!(!is_enabled_named(NAME).expect("03 is disabled"));

        write_approved_enabled(NAME).expect("re-enable");
        assert!(is_enabled_named(NAME).expect("re-enabled"));

        delete_named(NAME).expect("cleanup");
        assert!(!is_enabled_named(NAME).expect("gone"));
    }
}
