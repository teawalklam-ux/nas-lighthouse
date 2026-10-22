use crate::config::AppConfig;
use crate::drives::DriveMount;
use crate::events::{self, EventSignal, EventTracker};
use crate::model::{ActiveAccess, HealthEvent, ItemHealth, LinkState, MetricsSnapshot};
use crate::parse::parse_block;
use crate::state::Reducer;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Notify;

const SCRIPT: &str = include_str!("script.sh");
const MAX_FRAME_BYTES: usize = 1_048_576;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn ssh_executable() -> PathBuf {
    if let Ok(windir) = std::env::var("WINDIR") {
        let candidate = PathBuf::from(windir).join("System32/OpenSSH/ssh.exe");
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from("ssh")
}

const CONFIG_BEGIN: &str = "# === nas-lighthouse:config:begin ===";
const CONFIG_END: &str = "# === nas-lighthouse:config:end ===";

/// Single-quote a value for the remote shell. The script runs whatever we send,
/// so a stray quote in a user-supplied interface or mount name must not be able
/// to end the assignment and start a command.
fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// Replace the script's config block with the user's settings. Everything the
/// remote side needs is a plain assignment, so the script stays runnable by
/// hand with its own defaults.
pub fn patched_script(cfg: &AppConfig) -> String {
    let secs = (cfg.ui.interval_ms / 1000).clamp(1, 30);
    let collector = &cfg.collector;
    let assignments = [
        format!("INTERVAL={secs}"),
        format!("ENABLE_NET_OUT={}", u8::from(cfg.ui.internet_probe_enabled)),
        format!(
            "IFACE={}",
            sh_quote(collector.net_iface.as_deref().unwrap_or_default())
        ),
        format!(
            "MOUNTS=({})",
            collector
                .disk_mounts
                .iter()
                .map(|path| sh_quote(path))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        format!("PROBE_HOSTS={}", sh_quote(&collector.probe_hosts.join(" "))),
        format!(
            "LINK_AGENT={}",
            sh_quote(collector.link_agent_process.as_deref().unwrap_or_default())
        ),
    ];
    let block = format!("{CONFIG_BEGIN}\n{}\n{CONFIG_END}", assignments.join("\n"));

    let Some(start) = SCRIPT.find(CONFIG_BEGIN) else {
        return SCRIPT.to_string();
    };
    let Some(end) = SCRIPT[start..]
        .find(CONFIG_END)
        .map(|offset| start + offset + CONFIG_END.len())
    else {
        return SCRIPT.to_string();
    };
    format!("{}{}{}", &SCRIPT[..start], block, &SCRIPT[end..])
}

/// 别名作为裸参数交给 `ssh`，以 `-` 开头的值会被 OpenSSH 当成选项（如 `-oProxyCommand=…`）。
pub fn valid_ssh_alias(alias: &str) -> bool {
    if alias.is_empty() || alias.len() > 128 || alias.starts_with('-') {
        return false;
    }
    let (user, host) = match alias.split_once('@') {
        Some((user, host)) => (Some(user), host),
        None => (None, alias),
    };
    let plain = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    };
    user.map_or(true, plain) && plain(host)
}

pub fn spawn_ssh(alias: &str) -> Result<tokio::process::Child> {
    if !valid_ssh_alias(alias) {
        anyhow::bail!("SSH 别名不合法：{alias:?}");
    }
    let mut cmd = Command::new(ssh_executable());
    cmd.arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("RequestTTY=no")
        .arg("-o")
        .arg("TCPKeepAlive=yes")
        .arg("-o")
        .arg("ConnectTimeout=10")
        .arg("-o")
        .arg("ServerAliveInterval=15")
        .arg("-o")
        .arg("ServerAliveCountMax=3")
        .arg(alias)
        .arg("bash")
        .arg("-s")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn().context("spawn ssh")
}

pub(crate) fn normalize_remote_script(script: &str) -> String {
    script.replace("\r\n", "\n").replace('\r', "\n")
}

pub async fn write_script(stdin: &mut tokio::process::ChildStdin, script: &str) -> Result<()> {
    // 脚本按 LF 喂给远端 bash；CRLF 会让 `sleep "$INTERVAL"` 之类带上 \r。
    let script = normalize_remote_script(script);
    stdin.write_all(script.as_bytes()).await?;
    stdin.flush().await?;
    Ok(())
}

pub struct SharedSnapshot {
    metrics: Mutex<MetricsSnapshot>,
    access: Mutex<Vec<ActiveAccess>>,
    pub config: Mutex<AppConfig>,
    pub health: Mutex<HashMap<String, ItemHealth>>,
    pub fail_streak: Mutex<HashMap<String, u32>>,
    events: Mutex<EventTracker>,
    pub drives: Mutex<Vec<DriveMount>>,
    pub restart: Notify,
}

impl SharedSnapshot {
    pub fn new(config: AppConfig) -> Arc<Self> {
        Arc::new(Self {
            metrics: Mutex::new(MetricsSnapshot::empty()),
            access: Mutex::new(Vec::new()),
            config: Mutex::new(config),
            health: Mutex::new(HashMap::new()),
            fail_streak: Mutex::new(HashMap::new()),
            events: Mutex::new(EventTracker::new(events::load())),
            drives: Mutex::new(crate::drives::placeholders()),
            restart: Notify::new(),
        })
    }

    pub fn store_metrics(&self, snap: MetricsSnapshot) {
        if let Ok(mut guard) = self.metrics.lock() {
            *guard = snap;
        }
    }

    pub fn store_access(&self, access: Vec<ActiveAccess>) {
        if let Ok(mut guard) = self.access.lock() {
            *guard = access;
        }
    }

    pub fn set_link(&self, link: LinkState) {
        if let Ok(mut guard) = self.metrics.lock() {
            guard.link = link;
        }
    }

    pub fn get(&self) -> MetricsSnapshot {
        let mut snap = self
            .metrics
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| MetricsSnapshot::empty());
        snap.active_access = self.access.lock().map(|g| g.clone()).unwrap_or_default();
        snap
    }

    pub fn config(&self) -> AppConfig {
        self.config.lock().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn store_drives(&self, mounts: Vec<DriveMount>) {
        if let Ok(mut guard) = self.drives.lock() {
            *guard = mounts;
        }
    }

    pub fn get_drives(&self) -> Vec<DriveMount> {
        self.drives.lock().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn get_events(&self) -> Vec<HealthEvent> {
        self.events
            .lock()
            .map(|tracker| tracker.snapshot())
            .unwrap_or_default()
    }

    pub fn observe_events(&self, signals: Vec<EventSignal>) -> Option<Vec<HealthEvent>> {
        let next = {
            let mut tracker = self.events.lock().ok()?;
            if !tracker.observe(signals) {
                return None;
            }
            tracker.snapshot()
        };
        if let Err(err) = events::save(&next) {
            eprintln!("events: {err:#}");
        }
        Some(next)
    }
}

pub async fn run_stream_once(
    alias: &str,
    script: &str,
    reducer: &mut Reducer,
    mut on_snap: impl FnMut(MetricsSnapshot),
    stop_after: Option<u32>,
) -> Result<()> {
    reducer.reset_rates();
    let mut child = spawn_ssh(alias)?;
    // Windows OpenSSH 在 stdin 收到 EOF 时会拆掉整段会话。脚本写完后必须把 stdin 握到采集结束。
    let mut stdin = child.stdin.take().context("ssh stdin")?;
    write_script(&mut stdin, script).await?;
    let stderr_tail = Arc::new(Mutex::new(String::new()));
    if let Some(stderr) = child.stderr.take() {
        let buf = stderr_tail.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(mut guard) = buf.lock() {
                    if guard.len() > 4000 {
                        guard.clear();
                    }
                    if !guard.is_empty() {
                        guard.push('\n');
                    }
                    guard.push_str(&line);
                }
            }
        });
    }
    let stdout = child.stdout.take().context("ssh stdout")?;
    let mut reader = BufReader::new(stdout).lines();
    let mut acc = String::new();
    let mut frames = 0u32;
    let read_timeout = Duration::from_secs(45);

    loop {
        let line = tokio::time::timeout(read_timeout, reader.next_line()).await;
        let line = match line {
            Ok(Ok(Some(line))) => line,
            Ok(Ok(None)) => {
                let detail = stderr_tail
                    .lock()
                    .ok()
                    .map(|g| g.trim().to_string())
                    .filter(|s| !s.is_empty());
                let status = child.wait().await.ok();
                let mut msg = String::from("ssh stdout closed");
                if let Some(status) = status {
                    msg.push_str(&format!(" ({status})"));
                }
                if let Some(detail) = detail {
                    msg.push_str(": ");
                    msg.push_str(&detail);
                }
                drop(stdin);
                anyhow::bail!(msg);
            }
            Ok(Err(err)) => return Err(err.into()),
            Err(_) => anyhow::bail!("ssh collector timed out"),
        };
        if acc.len().saturating_add(line.len()).saturating_add(1) > MAX_FRAME_BYTES {
            acc.clear();
            if line.trim() != "=END" {
                continue;
            }
        } else {
            acc.push_str(&line);
            acc.push('\n');
        }
        if line.trim() == "=END" {
            if let Some(sample) = parse_block(&acc) {
                let snap = reducer.apply(sample, LinkState::Connected);
                frames += 1;
                on_snap(snap);
                if let Some(limit) = stop_after {
                    if frames >= limit {
                        let _ = child.kill().await;
                        return Ok(());
                    }
                }
            }
            acc.clear();
        }
    }
}

pub async fn run_forever(
    shared: Arc<SharedSnapshot>,
    emit: impl Fn(MetricsSnapshot) + Send + 'static,
) {
    let mut reducer = Reducer::new();
    let mut backoff = 1u64;
    loop {
        let cfg = shared.config();
        if !cfg.configured() {
            // Nothing to connect to yet. Stay idle instead of retrying an
            // empty alias in a loop; the UI is showing the setup prompt.
            shared.set_link(LinkState::Disconnected);
            emit(shared.get());
            shared.restart.notified().await;
            continue;
        }
        let script = patched_script(&cfg);
        shared.set_link(LinkState::Reconnecting);
        emit(shared.get());
        tokio::select! {
            result = run_stream_once(&cfg.ssh_alias, &script, &mut reducer, |snap| {
                backoff = 1;
                shared.store_metrics(snap);
                emit(shared.get());
            }, None) => {
                if let Err(err) = result {
                    eprintln!("collector: {err:#}");
                }
                reducer.reset_rates();
                shared.set_link(LinkState::Disconnected);
                emit(shared.get());
                tokio::time::sleep(Duration::from_secs(backoff)).await;
                backoff = (backoff.saturating_mul(2)).min(30);
            }
            _ = shared.restart.notified() => {
                reducer.reset_rates();
                backoff = 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_alias_and_user_at_host() {
        assert!(valid_ssh_alias("my-nas"));
        assert!(valid_ssh_alias("admin@192.0.2.10"));
        assert!(valid_ssh_alias("nas.local"));
    }

    #[test]
    fn patched_script_carries_collector_settings() {
        let mut cfg = AppConfig::default();
        cfg.ui.interval_ms = 5000;
        cfg.ui.internet_probe_enabled = false;
        cfg.collector.net_iface = Some("enp2s0".into());
        cfg.collector.disk_mounts = vec!["/volume1".into(), "/mnt/data".into()];
        cfg.collector.probe_hosts = vec!["9.9.9.9".into()];
        cfg.collector.link_agent_process = Some("nas_agent_servi".into());
        let script = patched_script(&cfg);
        assert!(script.contains("INTERVAL=5"));
        assert!(script.contains("ENABLE_NET_OUT=0"));
        assert!(script.contains("IFACE='enp2s0'"));
        assert!(script.contains("MOUNTS=('/volume1' '/mnt/data')"));
        assert!(script.contains("PROBE_HOSTS='9.9.9.9'"));
        assert!(script.contains("LINK_AGENT='nas_agent_servi'"));
        // The block is replaced, not appended.
        let config_block = script
            .split(CONFIG_BEGIN)
            .nth(1)
            .unwrap()
            .split(CONFIG_END)
            .next()
            .unwrap();
        assert_eq!(config_block.matches("IFACE=").count(), 1);
        assert!(!script.contains("INTERVAL=2\n"));
    }

    #[test]
    fn defaults_leave_detection_to_the_nas() {
        let script = patched_script(&AppConfig::default());
        assert!(script.contains("IFACE=''"));
        assert!(script.contains("MOUNTS=()"));
        assert!(script.contains("LINK_AGENT=''"));
        assert!(script.contains("PROBE_HOSTS='1.1.1.1 8.8.8.8'"));
    }

    #[test]
    fn collector_values_cannot_break_out_of_the_assignment() {
        let mut cfg = AppConfig::default();
        cfg.collector.net_iface = Some("eth0'; rm -rf /tmp; echo '".into());
        let script = patched_script(&cfg);
        assert!(script.contains(r"IFACE='eth0'\''; rm -rf /tmp; echo '\'''"));
        assert!(!script.contains("IFACE='eth0'; rm"));
    }

    #[test]
    fn rejects_option_looking_alias() {
        assert!(!valid_ssh_alias("-oProxyCommand=calc.exe"));
        assert!(!valid_ssh_alias("-i/tmp/key"));
    }

    #[test]
    fn rejects_shell_and_space_characters() {
        assert!(!valid_ssh_alias(""));
        assert!(!valid_ssh_alias("nas host"));
        assert!(!valid_ssh_alias("nas;whoami"));
        assert!(!valid_ssh_alias("a@b@c"));
        assert!(!valid_ssh_alias("@nas"));
    }

    #[test]
    fn spawn_refuses_invalid_alias() {
        assert!(spawn_ssh("-oProxyCommand=calc.exe").is_err());
    }

    #[test]
    fn strips_crlf_before_sending_to_bash() {
        assert_eq!(
            normalize_remote_script("INTERVAL=2\r\nsleep 1\r"),
            "INTERVAL=2\nsleep 1\n"
        );
    }
}
