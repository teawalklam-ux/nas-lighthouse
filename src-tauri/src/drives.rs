use crate::config::AppConfig;
use crate::model::now_unix_ms;
use crate::search;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::timeout;

const PROBE_TIMEOUT: Duration = Duration::from_secs(8);
const REMOUNT_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_CREDENTIAL: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriveMount {
    pub id: String,
    pub label: String,
    pub mount_path: String,
    pub state: String,
    pub status: String,
    pub message: String,
    pub checked_ts: u64,
    pub latency_ms: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct StorageItem {
    id: Option<i64>,
    driver: Option<String>,
    mount_path: Option<String>,
    status: Option<String>,
    disabled: Option<bool>,
}

fn targets(cfg: &AppConfig) -> [(&'static str, &'static str, &str); 2] {
    [
        ("quark", "夸克", &cfg.search.quark_mount),
        ("baidu", "百度", &cfg.search.baidu_mount),
    ]
}

pub fn placeholders() -> Vec<DriveMount> {
    targets(&AppConfig::default())
        .into_iter()
        .map(|(id, label, path)| DriveMount {
            id: id.into(),
            label: label.into(),
            mount_path: path.into(),
            state: "unknown".into(),
            status: String::new(),
            message: "尚未检查".into(),
            checked_ts: 0,
            latency_ms: None,
        })
        .collect()
}

fn http_client(timeout_dur: Duration) -> Result<Client, String> {
    Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(timeout_dur)
        .connect_timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败：{e}"))
}

async fn alist_json(
    client: &Client,
    method: reqwest::Method,
    url: &str,
    token: &str,
    body: Option<Value>,
) -> Result<Value, String> {
    let mut req = client.request(method, url).header(AUTHORIZATION, token);
    if let Some(payload) = body {
        req = req.header(CONTENT_TYPE, "application/json").json(&payload);
    }
    let resp = timeout(REMOUNT_TIMEOUT, req.send())
        .await
        .map_err(|_| "AList 请求超时".to_string())?
        .map_err(|e| format!("AList 请求失败：{e}"))?;
    let status = resp.status();
    let value: Value = resp
        .json()
        .await
        .map_err(|e| format!("AList 响应无法解析：{e}"))?;
    if status.as_u16() == 401 || value.get("code").and_then(Value::as_i64) == Some(401) {
        return Err("unauthorized".into());
    }
    Ok(value)
}

async fn with_token<F, Fut, T>(cfg: &AppConfig, mut work: F) -> Result<T, String>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let mut token = search::fetch_alist_token(cfg).await?;
    match work(token.clone()).await {
        Err(err) if err == "unauthorized" => {
            search::invalidate_token();
            token = search::fetch_alist_token(cfg).await?;
            work(token).await
        }
        other => other,
    }
}

fn is_bool_flag(value: &str) -> bool {
    matches!(value, "TRUE" | "FALSE" | "true" | "false")
}

fn push_cookie(cookies: &mut Vec<(String, String)>, name: String, value: String) {
    if name.is_empty() {
        return;
    }
    if let Some(existing) = cookies.iter_mut().find(|(key, _)| *key == name) {
        existing.1 = value;
    } else {
        cookies.push((name, value));
    }
}

fn parse_netscape_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    if line.contains('\t') {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() >= 7 && !cols[5].is_empty() {
            return Some((cols[5].to_string(), cols[6].to_string()));
        }
        return None;
    }
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 7 && is_bool_flag(parts[1]) && is_bool_flag(parts[3]) && !parts[5].is_empty()
    {
        return Some((parts[5].to_string(), parts[6..].join(" ")));
    }
    None
}

fn json_cookies_to_header(text: &str) -> Option<String> {
    let value: Value = serde_json::from_str(text).ok()?;
    let arr = value.as_array()?;
    let mut cookies: Vec<(String, String)> = Vec::new();
    for item in arr {
        let name = item.get("name")?.as_str()?.to_string();
        let cookie_value = item
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        push_cookie(&mut cookies, name, cookie_value);
    }
    if cookies.is_empty() {
        return None;
    }
    Some(join_cookies(cookies))
}

fn join_cookies(cookies: Vec<(String, String)>) -> String {
    cookies
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn cookie_from_credential(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("请粘贴夸克 Cookie".into());
    }
    if trimmed.len() > MAX_CREDENTIAL {
        return Err("Cookie 过长".into());
    }
    if let Some(header) = json_cookies_to_header(trimmed) {
        return Ok(header);
    }
    let header = netscape_to_cookie(trimmed);
    if !header.is_empty() {
        return Ok(header);
    }
    let header = trimmed
        .trim_start_matches(|ch: char| ch.is_ascii_whitespace())
        .strip_prefix("Cookie:")
        .or_else(|| trimmed.strip_prefix("cookie:"))
        .unwrap_or(trimmed)
        .trim()
        .replace(['\n', '\r'], "");
    if header.is_empty() {
        return Err("Cookie 文件无法解析".into());
    }
    Ok(header)
}

pub fn netscape_to_cookie(text: &str) -> String {
    let mut cookies: Vec<(String, String)> = Vec::new();
    for raw in text.lines() {
        if let Some((name, value)) = parse_netscape_line(raw) {
            push_cookie(&mut cookies, name, value);
        }
    }
    join_cookies(cookies)
}

pub fn refresh_token_from_credential(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("请粘贴百度 refresh_token".into());
    }
    if trimmed.len() > MAX_CREDENTIAL {
        return Err("refresh_token 过长".into());
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(token) = value.get("refresh_token").and_then(Value::as_str) {
            return ok_refresh_token(token);
        }
    }
    for line in trimmed.lines() {
        let line = line.trim().trim_matches(|ch| ch == '"' || ch == '\'');
        if let Some(rest) = line
            .strip_prefix("refresh_token")
            .or_else(|| line.strip_prefix("refreshToken"))
        {
            let token = rest
                .trim_start_matches([':', '=', ' ', '"', '\''])
                .trim_end_matches(['"', '\'', ',', ' '])
                .trim();
            if token.len() >= 16 {
                return ok_refresh_token(token);
            }
        }
    }
    ok_refresh_token(&trimmed.replace(['\n', '\r'], ""))
}

fn ok_refresh_token(token: &str) -> Result<String, String> {
    let token = token.trim();
    if token.len() < 16 {
        return Err("refresh_token 太短".into());
    }
    if token.len() > MAX_CREDENTIAL {
        return Err("refresh_token 过长".into());
    }
    Ok(token.to_string())
}

fn find_storage<'a>(items: &'a [StorageItem], mount_path: &str) -> Option<&'a StorageItem> {
    items
        .iter()
        .find(|item| item.mount_path.as_deref() == Some(mount_path))
}

fn parse_storage_list(value: &Value) -> Result<Vec<StorageItem>, String> {
    let code = value.get("code").and_then(Value::as_i64).unwrap_or(0);
    if code != 200 {
        let msg = value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("列出存储失败");
        return Err(msg.into());
    }
    let content = value
        .pointer("/data/content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut items = Vec::new();
    for row in content {
        items.push(StorageItem {
            id: row.get("id").and_then(Value::as_i64),
            driver: row
                .get("driver")
                .and_then(Value::as_str)
                .map(str::to_string),
            mount_path: row
                .get("mount_path")
                .and_then(Value::as_str)
                .map(str::to_string),
            status: row
                .get("status")
                .and_then(Value::as_str)
                .map(str::to_string),
            disabled: row.get("disabled").and_then(Value::as_bool),
        });
    }
    Ok(items)
}

fn alist_ok(value: &Value) -> Result<(), String> {
    let code = value.get("code").and_then(Value::as_i64).unwrap_or(0);
    if code == 200 {
        return Ok(());
    }
    Err(value
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("AList 操作失败")
        .into())
}

pub async fn probe_drives(cfg: &AppConfig) -> Vec<DriveMount> {
    match probe_drives_inner(cfg).await {
        Ok(mounts) => mounts,
        Err(err) => targets(cfg)
            .into_iter()
            .map(|(id, label, path)| DriveMount {
                id: id.into(),
                label: label.into(),
                mount_path: path.into(),
                state: "unknown".into(),
                status: String::new(),
                message: err.clone(),
                checked_ts: now_unix_ms(),
                latency_ms: None,
            })
            .collect(),
    }
}

async fn probe_drives_inner(cfg: &AppConfig) -> Result<Vec<DriveMount>, String> {
    if cfg.search.alist_url.trim().is_empty() {
        return Err("未配置 AList 地址".into());
    }
    search::validate_alist_url(&cfg.search.alist_url)?;
    let client = http_client(PROBE_TIMEOUT)?;
    let base = cfg.search.alist_url.trim_end_matches('/').to_string();
    let storages = with_token(cfg, |token| {
        let client = client.clone();
        let url = format!("{base}/api/admin/storage/list");
        async move {
            let value = alist_json(&client, reqwest::Method::GET, &url, &token, None).await?;
            parse_storage_list(&value)
        }
    })
    .await?;

    let mut out = Vec::new();
    for (id, label, path) in targets(cfg) {
        out.push(probe_one(&client, cfg, &storages, id, label, path).await);
    }
    Ok(out)
}

async fn probe_one(
    client: &Client,
    cfg: &AppConfig,
    storages: &[StorageItem],
    id: &str,
    label: &str,
    path: &str,
) -> DriveMount {
    let checked_ts = now_unix_ms();
    let Some(item) = find_storage(storages, path) else {
        return DriveMount {
            id: id.into(),
            label: label.into(),
            mount_path: path.into(),
            state: "missing".into(),
            status: String::new(),
            message: "尚未挂载".into(),
            checked_ts,
            latency_ms: None,
        };
    };
    let status = item.status.clone().unwrap_or_default();
    if item.disabled == Some(true) || status != "work" {
        return DriveMount {
            id: id.into(),
            label: label.into(),
            mount_path: path.into(),
            state: "down".into(),
            status: status.clone(),
            message: if status.is_empty() {
                "已停用".into()
            } else {
                status
            },
            checked_ts,
            latency_ms: None,
        };
    }
    let start = std::time::Instant::now();
    let listed = with_token(cfg, |token| {
        let client = client.clone();
        let url = format!("{}/api/fs/list", cfg.search.alist_url.trim_end_matches('/'));
        let body = json!({
            "path": path,
            "page": 1,
            "per_page": 1,
            "refresh": false
        });
        async move { alist_json(&client, reqwest::Method::POST, &url, &token, Some(body)).await }
    })
    .await;
    let latency_ms = start.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
    match listed {
        Ok(value) => match alist_ok(&value) {
            Ok(()) => DriveMount {
                id: id.into(),
                label: label.into(),
                mount_path: path.into(),
                state: "up".into(),
                status,
                message: "挂载正常".into(),
                checked_ts,
                latency_ms: Some(latency_ms),
            },
            Err(err) => DriveMount {
                id: id.into(),
                label: label.into(),
                mount_path: path.into(),
                state: "down".into(),
                status,
                message: err,
                checked_ts,
                latency_ms: Some(latency_ms),
            },
        },
        Err(err) => DriveMount {
            id: id.into(),
            label: label.into(),
            mount_path: path.into(),
            state: "down".into(),
            status,
            message: err,
            checked_ts,
            latency_ms: Some(latency_ms),
        },
    }
}

pub async fn remount_drive(
    cfg: &AppConfig,
    kind: &str,
    credential: &str,
) -> Result<Vec<DriveMount>, String> {
    search::validate_alist_url(&cfg.search.alist_url)?;
    let kind = kind.trim();
    let client = http_client(REMOUNT_TIMEOUT)?;
    let base = cfg.search.alist_url.trim_end_matches('/').to_string();
    match kind {
        "quark" => remount_quark(cfg, &client, &base, credential).await?,
        "baidu" => remount_baidu(cfg, &client, &base, credential).await?,
        _ => return Err("未知网盘".into()),
    }
    Ok(probe_drives(cfg).await)
}

async fn remount_quark(
    cfg: &AppConfig,
    client: &Client,
    base: &str,
    credential: &str,
) -> Result<(), String> {
    let cookie = cookie_from_credential(credential)?;
    update_credential(
        cfg,
        client,
        base,
        &cfg.search.quark_mount,
        "Quark",
        "cookie",
        &cookie,
    )
    .await
}

async fn remount_baidu(
    cfg: &AppConfig,
    client: &Client,
    base: &str,
    credential: &str,
) -> Result<(), String> {
    let refresh = refresh_token_from_credential(credential)?;
    update_credential(
        cfg,
        client,
        base,
        &cfg.search.baidu_mount,
        "BaiduNetdisk",
        "refresh_token",
        &refresh,
    )
    .await
}

/// Keep the complete server-side object, including unknown future settings.
/// Refuse missing/malformed addition instead of overwriting it with defaults.
fn credential_update(
    mut storage: Value,
    id: i64,
    mount: &str,
    driver: &str,
    field: &str,
    credential: &str,
) -> Result<Value, String> {
    if storage.get("id").and_then(Value::as_i64) != Some(id)
        || storage.get("mount_path").and_then(Value::as_str) != Some(mount)
        || storage.get("driver").and_then(Value::as_str) != Some(driver)
    {
        return Err("存储信息已变化，请刷新后重试".into());
    }
    let raw = storage
        .get("addition")
        .and_then(Value::as_str)
        .ok_or("无法读取完整存储配置，请在 AList 后台更新凭据")?;
    let mut addition: Value =
        serde_json::from_str(raw).map_err(|_| "存储配置无法解析，未作修改")?;
    let object = addition
        .as_object_mut()
        .ok_or("存储配置格式不正确，未作修改")?;
    object.insert(field.to_string(), Value::String(credential.to_string()));
    storage["addition"] = Value::String(addition.to_string());
    Ok(storage)
}

async fn update_credential(
    cfg: &AppConfig,
    client: &Client,
    base: &str,
    mount: &str,
    driver: &str,
    field: &str,
    credential: &str,
) -> Result<(), String> {
    let storages = with_token(cfg, |token| {
        let client = client.clone();
        let url = format!("{base}/api/admin/storage/list");
        async move {
            let value = alist_json(&client, reqwest::Method::GET, &url, &token, None).await?;
            parse_storage_list(&value)
        }
    })
    .await?;
    let item = find_storage(&storages, mount)
        .ok_or("请先在 AList 后台创建存储，并在设置中填写对应挂载路径")?;
    if item.driver.as_deref() != Some(driver) {
        return Err("挂载路径对应的网盘类型不匹配，未作修改".into());
    }
    let id = item.id.ok_or("存储缺少 ID，未作修改")?;
    let details = with_token(cfg, |token| {
        let client = client.clone();
        let url = format!("{base}/api/admin/storage/get?id={id}");
        async move { alist_json(&client, reqwest::Method::GET, &url, &token, None).await }
    })
    .await?;
    alist_ok(&details)?;
    let storage = details
        .get("data")
        .cloned()
        .ok_or("存储详情缺失，未作修改")?;
    let body = credential_update(storage, id, mount, driver, field, credential)?;
    let result = with_token(cfg, |token| {
        let client = client.clone();
        let url = format!("{base}/api/admin/storage/update");
        let payload = body.clone();
        async move { alist_json(&client, reqwest::Method::POST, &url, &token, Some(payload)).await }
    })
    .await?;
    alist_ok(&result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_netscape_cookies_last_name_wins() {
        let raw = "\
# Netscape HTTP Cookie File\n\
pan.quark.cn\tFALSE\t/\tFALSE\t0\tctoken\told\n\
.quark.cn\tTRUE\t/\tFALSE\t0\tctoken\tnew\n\
.quark.cn\tTRUE\t/\tFALSE\t0\t__puus\tabc\n";
        let header = netscape_to_cookie(raw);
        assert!(header.contains("ctoken=new"));
        assert!(!header.contains("ctoken=old"));
        assert!(header.contains("__puus=abc"));
    }

    #[test]
    fn converts_space_separated_netscape() {
        let raw = "# Netscape HTTP Cookie File\n\
.quark.cn TRUE / FALSE 0 ctoken new-token\n\
.quark.cn TRUE / FALSE 0 __puus abc";
        let header = netscape_to_cookie(raw);
        assert!(header.contains("ctoken=new-token"));
        assert!(header.contains("__puus=abc"));
    }

    #[test]
    fn converts_json_cookie_array() {
        let raw = r#"[{"name":"ctoken","value":"abc"},{"name":"__puus","value":"xyz"}]"#;
        let header = cookie_from_credential(raw).unwrap();
        assert_eq!(header, "ctoken=abc; __puus=xyz");
    }

    #[test]
    fn extracts_baidu_refresh_token_from_json() {
        let raw = r#"{"refresh_token":"122.abcdefghijklmnopqrstuvwxyz","access_token":"ignore"}"#;
        let token = refresh_token_from_credential(raw).unwrap();
        assert_eq!(token, "122.abcdefghijklmnopqrstuvwxyz");
    }

    #[test]
    fn credential_change_preserves_complete_storage() {
        let original = json!({"id": 8, "driver": "BaiduNetdisk", "mount_path": "/cloud/team",
            "web_proxy": true, "enable_sign": true, "remark": "keep", "disabled": true,
            "future_option": {"value": 42}, "addition":
            r#"{"refresh_token":"old","client_id":"example-id","client_secret":"example-secret","root_folder_path":"/photos"}"#});
        let next = credential_update(
            original.clone(),
            8,
            "/cloud/team",
            "BaiduNetdisk",
            "refresh_token",
            "replacement",
        )
        .unwrap();
        for (key, value) in original.as_object().unwrap() {
            if key != "addition" {
                assert_eq!(&next[key], value);
            }
        }
        let addition: Value = serde_json::from_str(next["addition"].as_str().unwrap()).unwrap();
        assert_eq!(addition["refresh_token"], "replacement");
        assert_eq!(addition["client_secret"], "example-secret");
        assert_eq!(addition["root_folder_path"], "/photos");
        assert!(
            credential_update(original.clone(), 8, "/cloud/team", "Quark", "cookie", "x").is_err()
        );
        let mut broken = original;
        broken["addition"] = json!("invalid");
        assert!(credential_update(
            broken,
            8,
            "/cloud/team",
            "BaiduNetdisk",
            "refresh_token",
            "x"
        )
        .is_err());
    }

    #[test]
    fn cookie_header_passthrough() {
        let header = cookie_from_credential("a=1; b=2").unwrap();
        assert_eq!(header, "a=1; b=2");
    }

    #[test]
    fn empty_cookie_rejected() {
        assert!(cookie_from_credential("   ").is_err());
    }

    #[test]
    fn parses_storage_list_and_finds_mount() {
        let value = json!({
            "code": 200,
            "data": {
                "content": [
                    {"id": 3, "driver": "Quark", "mount_path": "/夸克", "status": "work", "disabled": false},
                    {"id": 4, "driver": "BaiduNetdisk", "mount_path": "/百度", "status": "expired", "disabled": false}
                ]
            }
        });
        let items = parse_storage_list(&value).unwrap();
        assert_eq!(find_storage(&items, "/夸克").unwrap().id, Some(3));
        assert_eq!(
            find_storage(&items, "/百度").unwrap().status.as_deref(),
            Some("expired")
        );
        assert!(find_storage(&items, "/missing").is_none());
    }
}
