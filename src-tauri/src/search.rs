use crate::collector::{ssh_executable, valid_ssh_alias};
use crate::config::{self, AlistBackend, AppConfig, SearchConfig};
use crate::nav::valid_http_url;
use anyhow::{Context, Result};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::time::timeout;

const SEARCH_TIMEOUT: Duration = Duration::from_secs(8);
const ES_WAIT_MS: u64 = 30_000;
const ES_TIMEOUT: Duration = Duration::from_secs(35);
const HTTP_PROBE_TIMEOUT: Duration = Duration::from_millis(400);
const TOKEN_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const MAX_QUERY: usize = 200;
const MAX_PER_SOURCE: usize = 80;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub source: String,
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub open_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub query: String,
    pub hits: Vec<SearchHit>,
    pub warnings: Vec<String>,
}

struct TokenCache {
    key: String,
    token: String,
    fetched_at: Instant,
}

impl TokenCache {
    fn for_connection(&self, key: &str) -> Option<String> {
        (self.key == key && self.fetched_at.elapsed() < TOKEN_TTL && !self.token.is_empty())
            .then(|| self.token.clone())
    }
}

pub(crate) fn validate_alist_url(raw: &str) -> Result<String, String> {
    let url = reqwest::Url::parse(raw.trim()).map_err(|_| "AList 地址无效")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("AList 地址仅支持不带账号、查询参数和片段的 HTTP(S) 地址".into());
    }
    Ok(url.as_str().trim_end_matches('/').to_string())
}

pub(crate) fn connection_key(cfg: &AppConfig) -> String {
    let base = validate_alist_url(&cfg.search.alist_url)
        .unwrap_or_else(|_| cfg.search.alist_url.trim().to_string());
    format!(
        "{}\0{}\0{}\0{:?}",
        base,
        cfg.ssh_alias.trim(),
        cfg.search.alist_container.trim(),
        cfg.search.alist_backend
    )
}

fn token_cache() -> &'static Mutex<Option<TokenCache>> {
    static CACHE: OnceLock<Mutex<Option<TokenCache>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

pub fn valid_container_name(name: &str) -> bool {
    let name = name.trim();
    (1..128).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

pub fn classify_alist_path(parent: &str) -> &'static str {
    let p = parent.trim();
    let lower = p.to_lowercase();
    if lower.contains("quark") || p.contains("夸克") {
        "quark"
    } else if lower.contains("baidu") || p.contains("百度") {
        "baidu"
    } else {
        "nas"
    }
}

pub fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            i += 2;
            while i < bytes.len() && !bytes[i].is_ascii_alphabetic() {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

pub fn parse_admin_token(output: &str) -> Option<String> {
    for raw in strip_ansi(output).lines() {
        let line = raw.trim();
        let lower = line.to_ascii_lowercase();
        let Some(idx) = lower.rfind("token:") else {
            continue;
        };
        let value = line[idx + 6..].trim().trim_matches('"').trim_matches('\'');
        let token: String = value
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            .collect();
        if token.len() >= 8 {
            return Some(token);
        }
    }
    None
}

pub fn encode_path_seg(seg: &str) -> String {
    let mut out = String::new();
    for b in seg.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn alist_open_url(base: &str, parent: &str, name: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    let mut path = String::new();
    for part in parent.split('/').chain(std::iter::once(name)) {
        if part.is_empty() {
            continue;
        }
        path.push('/');
        path.push_str(&encode_path_seg(part));
    }
    if path.is_empty() {
        format!("{base}/")
    } else {
        format!("{base}{path}")
    }
}

pub fn parse_everything_json(body: &str, query: &str) -> Result<Vec<SearchHit>, String> {
    let value: Value =
        serde_json::from_str(body).map_err(|e| format!("Everything 返回无法解析：{e}"))?;
    let results = value
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| "Everything 没有返回结果列表".to_string())?;
    let mut hits = Vec::new();
    for item in results.iter().take(MAX_PER_SOURCE) {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let dir = item.get("path").and_then(Value::as_str).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let path = if dir.is_empty() {
            name.clone()
        } else {
            format!("{dir}\\{name}")
        };
        let kind = item.get("type").and_then(Value::as_str).unwrap_or("file");
        hits.push(SearchHit {
            source: "local".into(),
            name,
            path,
            is_dir: kind.eq_ignore_ascii_case("folder") || kind.eq_ignore_ascii_case("dir"),
            size: item.get("size").and_then(Value::as_u64),
            open_url: None,
        });
    }
    let _ = query;
    Ok(hits)
}

#[allow(dead_code)]
pub fn parse_es_lines(output: &str) -> Vec<SearchHit> {
    let mut hits = Vec::new();
    for line in output.lines() {
        let path = line.trim();
        if path.is_empty() {
            continue;
        }
        let name = Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path)
            .to_string();
        hits.push(SearchHit {
            source: "local".into(),
            name,
            path: path.to_string(),
            is_dir: false,
            size: None,
            open_url: None,
        });
        if hits.len() >= MAX_PER_SOURCE {
            break;
        }
    }
    hits
}

pub fn parse_alist_search(body: &Value, alist_url: &str) -> Result<Vec<SearchHit>, String> {
    let code = body.get("code").and_then(Value::as_u64).unwrap_or(0);
    if code != 200 {
        let msg = body
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("AList 搜索失败");
        return Err(msg.to_string());
    }
    let content = body
        .pointer("/data/content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut hits = Vec::new();
    for item in content.iter().take(MAX_PER_SOURCE) {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let parent = item.get("parent").and_then(Value::as_str).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let path = if parent.is_empty() || parent == "/" {
            format!("/{name}")
        } else {
            format!("{}/{}", parent.trim_end_matches('/'), name)
        };
        hits.push(SearchHit {
            source: classify_alist_path(parent).to_string(),
            name: name.clone(),
            path: path.clone(),
            is_dir: item.get("is_dir").and_then(Value::as_bool).unwrap_or(false),
            size: item.get("size").and_then(Value::as_u64),
            open_url: Some(alist_open_url(alist_url, parent, &name)),
        });
    }
    Ok(hits)
}

fn http_client() -> Result<Client> {
    Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(SEARCH_TIMEOUT)
        .connect_timeout(Duration::from_secs(3))
        .build()
        .context("创建 HTTP 客户端失败")
}

fn everything_candidates(cfg: &SearchConfig) -> Vec<String> {
    let mut urls = Vec::new();
    let primary = cfg.everything_http.trim();
    if valid_http_url(primary) {
        urls.push(primary.trim_end_matches('/').to_string());
    }
    urls
}

pub fn parse_http_host_port(base: &str) -> Option<(String, u16)> {
    let trimmed = base.trim();
    let (rest, default_port) = if let Some(rest) = trimmed.strip_prefix("https://") {
        (rest, 443u16)
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        (rest, 80u16)
    } else {
        return None;
    };
    let hostport = rest.split('/').next().unwrap_or("").trim();
    if hostport.is_empty() {
        return None;
    }
    if let Some(ipv6) = hostport.strip_prefix('[') {
        let (host, tail) = ipv6.split_once(']')?;
        if tail.is_empty() {
            return Some((host.to_string(), default_port));
        }
        let port = tail.strip_prefix(':')?.parse().ok()?;
        return Some((host.to_string(), port));
    }
    if let Some((host, port)) = hostport.rsplit_once(':') {
        if host.contains(':') {
            return None;
        }
        return Some((host.to_string(), port.parse().ok()?));
    }
    Some((hostport.to_string(), default_port))
}

async fn everything_http_reachable(base: &str) -> bool {
    let Some((host, port)) = parse_http_host_port(base) else {
        return false;
    };
    timeout(
        HTTP_PROBE_TIMEOUT,
        tokio::net::TcpStream::connect((host.as_str(), port)),
    )
    .await
    .ok()
    .and_then(Result::ok)
    .is_some()
}

fn push_existing(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if path.is_file() && !paths.contains(&path) {
        paths.push(path);
    }
}

fn es_candidates(cfg: &SearchConfig) -> Vec<PathBuf> {
    if !cfg.everything_executable.trim().is_empty() {
        let mut paths = Vec::new();
        push_existing(&mut paths, PathBuf::from(cfg.everything_executable.trim()));
        return paths;
    }
    let mut paths = Vec::new();
    for root in ["ProgramFiles", "ProgramFiles(x86)"] {
        let Ok(root) = std::env::var(root) else {
            continue;
        };
        for name in ["Everything", "Everything 1.5a"] {
            push_existing(&mut paths, Path::new(&root).join(name).join("es.exe"));
        }
    }
    push_existing(&mut paths, config::config_dir().join("bin").join("es.exe"));
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            push_existing(&mut paths, dir.join("es.exe"));
        }
    }
    paths
}

pub fn parse_es_json(body: &str) -> Result<Vec<SearchHit>, String> {
    let body = body.trim_start_matches('\u{feff}').trim();
    if body.is_empty() || body == "[]" {
        return Ok(Vec::new());
    }
    let value: Value =
        serde_json::from_str(body).map_err(|e| format!("es.exe JSON 无法解析：{e}"))?;
    let results = value
        .as_array()
        .ok_or_else(|| "es.exe JSON 不是数组".to_string())?;
    let mut hits = Vec::new();
    for item in results.iter().take(MAX_PER_SOURCE) {
        let raw = item
            .get("filename")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if raw.is_empty() {
            continue;
        }
        let is_dir = raw.ends_with('\\') || raw.ends_with('/');
        let path = raw.trim_end_matches(['\\', '/']).to_string();
        let name = Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&path)
            .to_string();
        hits.push(SearchHit {
            source: "local".into(),
            name,
            path,
            is_dir,
            size: item.get("size").and_then(Value::as_u64),
            open_url: None,
        });
    }
    Ok(hits)
}

async fn everything_http_search(base: &str, query: &str) -> Result<Vec<SearchHit>, String> {
    let client = http_client().map_err(|e| e.to_string())?;
    let url = format!(
        "{}/?search={}&json=1&path_column=1&size_column=1&count={}",
        base.trim_end_matches('/'),
        urlencoding_query(query),
        MAX_PER_SOURCE
    );
    let resp = timeout(SEARCH_TIMEOUT, client.get(&url).send())
        .await
        .map_err(|_| format!("Everything HTTP 超时（{base}）"))?
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("error sending request") || msg.contains("Connection refused") {
                format!("Everything HTTP 未开启（{base}）")
            } else {
                format!("Everything HTTP 失败：{msg}")
            }
        })?;
    if !resp.status().is_success() {
        return Err(format!("Everything HTTP {}", resp.status()));
    }
    let body = resp.text().await.map_err(|e| e.to_string())?;
    parse_everything_json(&body, query)
}

fn urlencoding_query(query: &str) -> String {
    encode_path_seg(query).replace("%20", "+")
}

fn es_failure(stderr: &str, fallback: &str) -> String {
    let err = stderr.trim();
    if err.is_empty() {
        fallback.to_string()
    } else {
        err.to_string()
    }
}

async fn everything_es_search(cfg: &SearchConfig, query: &str) -> Result<Vec<SearchHit>, String> {
    let exe = es_candidates(cfg)
        .into_iter()
        .next()
        .ok_or_else(|| {
            "未找到 es.exe。请把官方命令行放到 Everything 目录，或 %APPDATA%\\nas-lighthouse\\bin\\es.exe，或加入 PATH。便携版默认不开 HTTP。".to_string()
        })?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!(
        "nas-lighthouse-es-{}-{stamp}.json",
        std::process::id()
    ));
    let mut cmd = Command::new(&exe);
    if let Some(dir) = exe.parent() {
        if dir.join("Everything.exe").is_file() || dir.join("everything.exe").is_file() {
            cmd.current_dir(dir);
        }
    }
    cmd.arg("-n")
        .arg(MAX_PER_SOURCE.to_string())
        .arg("-timeout")
        .arg(ES_WAIT_MS.to_string())
        .arg("-ipc1")
        .arg("-export-json")
        .arg(&tmp)
        .arg("-utf8-bom")
        .arg(query)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = timeout(ES_TIMEOUT, cmd.output())
        .await
        .map_err(|_| {
            "Everything 数据库仍在加载或 IPC 无响应。请确认 Everything 已打开，且没有用不同的管理员权限运行。".to_string()
        })?
        .map_err(|e| format!("启动 es.exe 失败：{e}"))?;
    let body = std::fs::read_to_string(&tmp).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp);
    if body.trim().is_empty() {
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(es_failure(
                &stderr,
                "es.exe 未返回结果，请确认 Everything 正在运行",
            ));
        }
        return Ok(Vec::new());
    }
    parse_es_json(&body)
}

async fn search_everything(cfg: &SearchConfig, query: &str) -> Result<Vec<SearchHit>, String> {
    match everything_es_search(cfg, query).await {
        Ok(hits) => return Ok(hits),
        Err(es_err) => {
            let mut last = es_err;
            for base in everything_candidates(cfg) {
                if !everything_http_reachable(&base).await {
                    continue;
                }
                match everything_http_search(&base, query).await {
                    Ok(hits) => return Ok(hits),
                    Err(err) => last = format!("{last}；{err}"),
                }
            }
            Err(last)
        }
    }
}

async fn ssh_output(alias: &str, remote: &str) -> Result<String, String> {
    if !valid_ssh_alias(alias) {
        return Err("SSH 别名不合法".into());
    }
    let mut cmd = Command::new(ssh_executable());
    cmd.arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("ConnectTimeout=10")
        .arg(alias)
        .arg(remote)
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = timeout(Duration::from_secs(20), cmd.output())
        .await
        .map_err(|_| "读取 AList 令牌超时".to_string())?
        .map_err(|e| format!("SSH 失败：{e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if !out.status.success() && stdout.trim().is_empty() {
        return Err(if stderr.trim().is_empty() {
            "SSH 没有输出".into()
        } else {
            stderr.trim().to_string()
        });
    }
    Ok(format!("{stdout}\n{stderr}"))
}

pub(crate) async fn fetch_alist_token(cfg: &AppConfig) -> Result<String, String> {
    validate_alist_url(&cfg.search.alist_url)?;
    let key = connection_key(cfg);
    if let Ok(guard) = token_cache().lock() {
        if let Some(token) = guard.as_ref().and_then(|cache| cache.for_connection(&key)) {
            return Ok(token);
        }
    }
    if !valid_container_name(&cfg.search.alist_container) {
        return Err("AList 容器名不合法".into());
    }
    let (directory, binary) = match cfg.search.alist_backend {
        AlistBackend::Alist => ("/opt/alist", "alist"),
        AlistBackend::Openlist => ("/opt/openlist", "openlist"),
    };
    let remote = format!(
        "docker exec -w {directory} {} ./{binary} admin token",
        cfg.search.alist_container.trim()
    );
    let output = ssh_output(&cfg.ssh_alias, &remote).await?;
    let token =
        parse_admin_token(&output).ok_or_else(|| "无法从 AList 读取管理员令牌".to_string())?;
    if let Ok(mut guard) = token_cache().lock() {
        *guard = Some(TokenCache {
            key,
            token: token.clone(),
            fetched_at: Instant::now(),
        });
    }
    Ok(token)
}

pub(crate) fn invalidate_token() {
    if let Ok(mut guard) = token_cache().lock() {
        *guard = None;
    }
}

async fn alist_search_once(
    client: &Client,
    alist_url: &str,
    token: &str,
    query: &str,
) -> Result<Vec<SearchHit>, String> {
    let url = format!("{}/api/fs/search", alist_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "parent": "/",
        "keywords": query,
        "scope": 0,
        "page": 1,
        "per_page": MAX_PER_SOURCE,
        "password": ""
    });
    let resp = timeout(
        SEARCH_TIMEOUT,
        client
            .post(&url)
            .header(AUTHORIZATION, token)
            .header(CONTENT_TYPE, "application/json")
            .json(&body)
            .send(),
    )
    .await
    .map_err(|_| "AList 搜索超时".to_string())?
    .map_err(|e| format!("AList 搜索失败：{e}"))?;
    let status = resp.status();
    let value: Value = resp
        .json()
        .await
        .map_err(|e| format!("AList 响应无法解析：{e}"))?;
    if status.as_u16() == 401 {
        return Err("unauthorized".into());
    }
    parse_alist_search(&value, alist_url)
}

async fn search_alist(cfg: &AppConfig, query: &str) -> Result<Vec<SearchHit>, String> {
    if cfg.search.alist_url.trim().is_empty() {
        return Ok(Vec::new());
    }
    validate_alist_url(&cfg.search.alist_url)?;
    let client = http_client().map_err(|e| e.to_string())?;
    let mut token = fetch_alist_token(cfg).await?;
    match alist_search_once(&client, &cfg.search.alist_url, &token, query).await {
        Err(err) if err == "unauthorized" => {
            invalidate_token();
            token = fetch_alist_token(cfg).await?;
            alist_search_once(&client, &cfg.search.alist_url, &token, query).await
        }
        other => other,
    }
}

pub async fn run_search(cfg: &AppConfig, query: &str) -> Result<SearchResponse, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("请输入关键词".into());
    }
    if query.len() > MAX_QUERY {
        return Err("关键词过长".into());
    }
    let everything = search_everything(&cfg.search, query);
    let alist = search_alist(cfg, query);
    let (local, remote) = tokio::join!(everything, alist);
    let mut hits = Vec::new();
    let mut warnings = Vec::new();
    match local {
        Ok(mut items) => hits.append(&mut items),
        Err(err) => warnings.push(format!("本机：{err}")),
    }
    match remote {
        Ok(mut items) => hits.append(&mut items),
        Err(err) => warnings.push(format!("AList：{err}")),
    }
    if hits.is_empty() && !warnings.is_empty() {
        return Err(warnings.join("；"));
    }
    Ok(SearchResponse {
        query: query.to_string(),
        hits,
        warnings,
    })
}

pub fn open_local_path(path: &str) -> Result<(), String> {
    let path = local_path_for_open(path)?.to_path_buf();
    if !path.exists() {
        return Err(format!("找不到文件：{}", path.display()));
    }
    if path.is_dir() {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("无法打开文件夹：{e}"))?;
        return Ok(());
    }
    tauri_plugin_opener::open_path(&path, None::<&str>)
        .map_err(|e| format!("无法打开文件：{}（{}）", path.display(), e))
}

pub fn reveal_local_path(path: &str) -> Result<(), String> {
    let path = local_path_for_open(path)?.to_path_buf();
    if !path.exists() {
        return Err(format!("找不到文件：{}", path.display()));
    }
    let mut select = std::ffi::OsString::from("/select,");
    select.push(path.as_os_str());
    let mut cmd = std::process::Command::new("explorer");
    cmd.arg(select);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn()
        .map_err(|e| format!("无法打开所在文件夹：{e}"))?;
    Ok(())
}

fn local_path_for_open(path: &str) -> Result<&Path, String> {
    let path = path.trim();
    if path.is_empty() || path.contains('\0') || path.chars().any(|c| c.is_control()) {
        return Err("路径无效".into());
    }
    let looks_windows = Path::new(path).is_absolute()
        || (path.len() >= 3 && path.as_bytes()[1] == b':')
        || path.starts_with("\\\\");
    if !looks_windows {
        return Err("只能打开本机路径".into());
    }
    Ok(Path::new(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn classifies_cloud_and_nas_paths() {
        assert_eq!(classify_alist_path("/夸克/电影"), "quark");
        assert_eq!(classify_alist_path("/quark/inbox"), "quark");
        assert_eq!(classify_alist_path("/百度/文档"), "baidu");
        assert_eq!(classify_alist_path("/NAS/影视"), "nas");
        assert_eq!(classify_alist_path("/photos"), "nas");
    }

    #[test]
    fn parses_admin_token_from_logrus() {
        let out = "\u{1b}[36mINFO\u{1b}[0m[2026-09-16 04:42:42] Admin token: abcdefghijklmnop";
        assert_eq!(parse_admin_token(out).as_deref(), Some("abcdefghijklmnop"));
    }

    #[test]
    fn builds_encoded_alist_url() {
        let url = alist_open_url("http://192.0.2.10:5244/", "/NAS/影视", "片.mp4");
        assert!(url.starts_with("http://192.0.2.10:5244/NAS/"));
        assert!(url.contains("%E5%BD%B1%E8%A7%86"));
        assert!(url.contains("mp4"));
    }

    #[test]
    fn parses_everything_and_alist_hits() {
        let everything = r#"{"totalResults":1,"results":[{"type":"file","name":"a.txt","path":"C:\\Temp","size":12}]}"#;
        let hits = parse_everything_json(everything, "a").unwrap();
        assert_eq!(hits[0].source, "local");
        assert_eq!(hits[0].path, "C:\\Temp\\a.txt");

        let body = json!({
            "code": 200,
            "message": "success",
            "data": {
                "content": [{
                    "parent": "/NAS/文档",
                    "name": "合同.pdf",
                    "is_dir": false,
                    "size": 100
                }],
                "total": 1
            }
        });
        let remote = parse_alist_search(&body, "http://192.0.2.10:5244").unwrap();
        assert_eq!(remote[0].source, "nas");
        assert_eq!(remote[0].path, "/NAS/文档/合同.pdf");
        assert!(remote[0].open_url.as_ref().unwrap().contains("/NAS/"));
    }

    #[test]
    fn parse_es_lines_takes_basename() {
        let hits = parse_es_lines("C:\\\\Movies\\\\foo.mkv\n");
        assert_eq!(hits[0].name, "foo.mkv");
        assert_eq!(hits[0].source, "local");
    }

    #[test]
    fn parse_es_json_reads_unicode_and_folders() {
        let body = "\u{feff}[{\"filename\":\"D:\\\\Projects\\\\示例项目\\\\\"}]\n";
        let hits = parse_es_json(body).unwrap();
        assert_eq!(hits[0].name, "示例项目");
        assert_eq!(hits[0].path, r"D:\Projects\示例项目");
        assert!(hits[0].is_dir);
    }

    #[test]
    fn parse_es_json_accepts_stdout_array() {
        let body = r#"[{"filename":"D:\\Projects\\notes\\示例.txt"}]"#;
        let hits = parse_es_json(body).unwrap();
        assert_eq!(hits[0].name, "示例.txt");
        assert!(!hits[0].is_dir);
    }

    #[test]
    fn parses_everything_http_host_port() {
        assert_eq!(
            parse_http_host_port("http://127.0.0.1:80"),
            Some(("127.0.0.1".into(), 80))
        );
        assert_eq!(
            parse_http_host_port("http://127.0.0.1:8080/"),
            Some(("127.0.0.1".into(), 8080))
        );
        assert_eq!(
            parse_http_host_port("https://localhost/search"),
            Some(("localhost".into(), 443))
        );
        assert_eq!(parse_http_host_port("not-a-url"), None);
    }

    #[test]
    fn rejects_bad_container_names() {
        assert!(valid_container_name("alist"));
        assert!(valid_container_name("stack_alist-1"));
        assert!(!valid_container_name(""));
        assert!(!valid_container_name("alist;rm"));
        assert!(!valid_container_name("alist 1"));
    }

    #[test]
    fn admin_cache_never_crosses_connection_boundaries() {
        let mut original = AppConfig::default();
        original.ssh_alias = "nas-a".into();
        original.search.alist_url = "https://files.example/".into();
        let cached = TokenCache {
            key: connection_key(&original),
            token: "synthetic-test-value".into(),
            fetched_at: Instant::now(),
        };
        assert!(cached.for_connection(&connection_key(&original)).is_some());
        let mut changed = original.clone();
        changed.search.alist_url = "https://other.example".into();
        assert!(cached.for_connection(&connection_key(&changed)).is_none());
        changed = original.clone();
        changed.ssh_alias = "nas-b".into();
        assert!(cached.for_connection(&connection_key(&changed)).is_none());
        changed = original.clone();
        changed.search.alist_container = "other-container".into();
        assert!(cached.for_connection(&connection_key(&changed)).is_none());
        changed = original.clone();
        changed.search.alist_backend = AlistBackend::Openlist;
        assert!(cached.for_connection(&connection_key(&changed)).is_none());
        let expired = TokenCache {
            fetched_at: Instant::now() - TOKEN_TTL - Duration::from_secs(1),
            ..cached
        };
        assert!(expired.for_connection(&connection_key(&original)).is_none());
    }

    #[test]
    fn admin_endpoint_rejects_embedded_credentials() {
        assert!(validate_alist_url("https://user:example@files.example/").is_err());
        assert!(validate_alist_url("https://files.example/?token=example").is_err());
        assert!(validate_alist_url("file:///tmp/config").is_err());
        assert_eq!(
            validate_alist_url("https://files.example/alist/").unwrap(),
            "https://files.example/alist"
        );
    }
}
