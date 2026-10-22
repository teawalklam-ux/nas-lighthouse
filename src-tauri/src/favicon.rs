use crate::nav::{host_port_from_http_url, valid_http_url};
use base64::Engine;
use reqwest::header::CONTENT_TYPE;
use reqwest::redirect::Policy;
use reqwest::{Client, Url};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::time::timeout;

const MAX_BYTES: usize = 128 * 1024;
const FETCH_TIMEOUT: Duration = Duration::from_secs(2);

static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

enum Body {
    Image(String),
    Html(String),
    Other,
}

pub fn cache_key(url: &str) -> String {
    origin_of(url).unwrap_or_else(|| url.trim().to_string())
}

pub fn origin_of(page_url: &str) -> Option<String> {
    let https = page_url.trim().starts_with("https://");
    let (host, port) = host_port_from_http_url(page_url)?;
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host
    };
    Some(match (https, port) {
        (true, 443) => format!("https://{host}"),
        (false, 80) => format!("http://{host}"),
        (true, p) => format!("https://{host}:{p}"),
        (false, p) => format!("http://{host}:{p}"),
    })
}

pub fn well_known_icon_urls(page_url: &str) -> Vec<String> {
    let Some(origin) = origin_of(page_url) else {
        return Vec::new();
    };
    ["/apple-touch-icon.png", "/favicon.png", "/favicon.ico"]
        .into_iter()
        .map(|path| format!("{origin}{path}"))
        .collect()
}

pub fn html_icon_hrefs(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut hrefs = Vec::new();
    let mut search = 0;
    while let Some(rel) = lower[search..].find("<link") {
        let start = search + rel;
        let Some(end) = lower[start..].find('>') else {
            break;
        };
        let tag = &html[start..start + end];
        let tag_l = &lower[start..start + end];
        search = start + 5;
        if !tag_l.contains("rel=") || !tag_l.contains("icon") || tag_l.contains("mask-icon") {
            continue;
        }
        if let Some(href) = attr(tag, "href") {
            let href = href.trim();
            if !href.is_empty() {
                hrefs.push(href.to_string());
            }
        }
    }
    hrefs
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let key = format!("{name}=");
    let pos = lower.find(&key)?;
    let rest = tag[pos + key.len()..].trim_start();
    match rest.as_bytes().first()? {
        b'"' => rest[1..].find('"').map(|i| rest[1..1 + i].to_string()),
        b'\'' => rest[1..].find('\'').map(|i| rest[1..1 + i].to_string()),
        _ => rest
            .split(|c: char| c.is_ascii_whitespace() || c == '>')
            .next()
            .map(|s| s.to_string()),
    }
}

pub fn sniff_image(bytes: &[u8], content_type: Option<&str>) -> Option<&'static str> {
    let bytes = trim_start(bytes);
    if bytes.len() < 4 || looks_html(bytes) {
        return None;
    }
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF8") {
        return Some("image/gif");
    }
    if bytes.starts_with(b"RIFF") && bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) || bytes.starts_with(&[0x00, 0x00, 0x02, 0x00])
    {
        return Some("image/x-icon");
    }
    if bytes.starts_with(b"<svg")
        || bytes.starts_with(b"<?xml") && bytes.windows(4).any(|w| w == b"<svg")
    {
        return Some("image/svg+xml");
    }
    let ct = content_type.unwrap_or("").to_ascii_lowercase();
    if ct.starts_with("image/") && !ct.contains("svg+xml") {
        if ct.contains("png") {
            return Some("image/png");
        }
        if ct.contains("jpeg") || ct.contains("jpg") {
            return Some("image/jpeg");
        }
        if ct.contains("gif") {
            return Some("image/gif");
        }
        if ct.contains("webp") {
            return Some("image/webp");
        }
        if ct.contains("icon") {
            return Some("image/x-icon");
        }
    }
    None
}

fn trim_start(bytes: &[u8]) -> &[u8] {
    let n = bytes.iter().take_while(|b| b.is_ascii_whitespace()).count();
    &bytes[n..]
}

fn looks_html(bytes: &[u8]) -> bool {
    let b = trim_start(bytes);
    b.starts_with(b"<!")
        || b.starts_with(b"<html")
        || b.starts_with(b"<HTML")
        || b.starts_with(b"<head")
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .no_proxy()
            .timeout(FETCH_TIMEOUT)
            .redirect(Policy::none())
            .user_agent("nas-lighthouse/0.1")
            .build()
            .expect("http client")
    })
}

fn join_http(base: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }
    let url = Url::parse(base).ok()?.join(href).ok()?;
    let base = Url::parse(base).ok()?;
    (matches!(url.scheme(), "http" | "https")
        && url.origin() == base.origin()
        && url.username().is_empty()
        && url.password().is_none())
    .then(|| url.into())
}

async fn get(url: &str) -> Body {
    let Ok(parsed) = Url::parse(url) else {
        return Body::Other;
    };
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Body::Other;
    }
    let send = client().get(url).send();
    let Ok(Ok(resp)) = timeout(FETCH_TIMEOUT, send).await else {
        return Body::Other;
    };
    if !resp.status().is_success() {
        return Body::Other;
    }
    if resp.content_length().is_some_and(|n| n > MAX_BYTES as u64) {
        return Body::Other;
    }
    let ct = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let Ok(Ok(bytes)) = timeout(FETCH_TIMEOUT, resp.bytes()).await else {
        return Body::Other;
    };
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Body::Other;
    }
    if let Some(mime) = sniff_image(&bytes, Some(&ct)) {
        return Body::Image(data_url(mime, &bytes));
    }
    let ct_l = ct.to_ascii_lowercase();
    if ct_l.contains("text/html") || ct_l.contains("application/xhtml") || looks_html(&bytes) {
        return Body::Html(String::from_utf8_lossy(&bytes).into_owned());
    }
    Body::Other
}

async fn first_image(urls: impl IntoIterator<Item = String>) -> Option<String> {
    let mut seen = Vec::new();
    for url in urls {
        if seen.iter().any(|u| u == &url) {
            continue;
        }
        seen.push(url.clone());
        if let Body::Image(data) = get(&url).await {
            return Some(data);
        }
    }
    None
}

pub async fn fetch(page_url: &str) -> Option<String> {
    let page_url = page_url.trim();
    if !valid_http_url(page_url) {
        return None;
    }
    match get(page_url).await {
        Body::Image(data) => return Some(data),
        Body::Html(html) => {
            let hrefs = html_icon_hrefs(&html)
                .into_iter()
                .filter_map(|href| join_http(page_url, &href));
            if let Some(data) = first_image(hrefs).await {
                return Some(data);
            }
        }
        Body::Other => {}
    }
    if let Some(data) = first_image(well_known_icon_urls(page_url)).await {
        return Some(data);
    }
    None
}

pub async fn fetch_cached(url: &str) -> Option<String> {
    let key = cache_key(url);
    if let Ok(guard) = cache().lock() {
        if let Some(hit) = guard.get(&key) {
            return hit.clone();
        }
    }
    let found = fetch(url).await;
    if let Ok(mut guard) = cache().lock() {
        guard.insert(key, found.clone());
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn icon_links_stay_on_the_service_origin() {
        let base = "http://nas.home.arpa:5244/";
        assert_eq!(
            join_http(base, "/icon.png").as_deref(),
            Some("http://nas.home.arpa:5244/icon.png")
        );
        assert!(join_http(base, "https://cdn.example/icon.png").is_none());
        assert!(join_http(base, "//nas.home.arpa:9000/icon.png").is_none());
        assert!(join_http(base, "http://user:pass@nas.home.arpa:5244/icon.png").is_none());
    }

    #[test]
    fn well_known_from_origin() {
        let urls = well_known_icon_urls("http://192.168.0.10:8096/web/");
        assert_eq!(
            urls,
            vec![
                "http://192.168.0.10:8096/apple-touch-icon.png",
                "http://192.168.0.10:8096/favicon.png",
                "http://192.168.0.10:8096/favicon.ico",
            ]
        );
    }

    #[test]
    fn html_picks_icon_hrefs() {
        let html = r#"
            <link rel="stylesheet" href="/app.css">
            <link rel="mask-icon" href="/mask.svg">
            <link rel="icon" href="/web/favicon.ico">
            <link rel="apple-touch-icon" href="https://cdn.example/a.png">
        "#;
        assert_eq!(
            html_icon_hrefs(html),
            vec!["/web/favicon.ico", "https://cdn.example/a.png"]
        );
    }

    #[test]
    fn sniff_rejects_html() {
        assert!(sniff_image(b"<!DOCTYPE html><html>", Some("image/png")).is_none());
        assert_eq!(sniff_image(PNG, None), Some("image/png"));
        assert_eq!(
            sniff_image(&[0x00, 0x00, 0x01, 0x00, 0x01], Some("image/x-icon")),
            Some("image/x-icon")
        );
    }

    #[test]
    fn cache_key_is_host_port() {
        assert_eq!(
            cache_key("http://192.168.0.10:8096/app"),
            "http://192.168.0.10:8096"
        );
        assert_eq!(cache_key("https://github.com/foo"), "https://github.com");
    }

    async fn serve(routes: &'static [(&'static str, &'static str, &'static [u8])]) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 2048];
                    let n = stream.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let path = req.split_whitespace().nth(1).unwrap_or("/");
                    let (status, ct, body) = routes
                        .iter()
                        .find(|(p, _, _)| *p == path)
                        .map(|(_, ct, body)| ("200 OK", *ct, *body))
                        .unwrap_or(("404 Not Found", "text/plain", b"no".as_slice()));
                    let head = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(head.as_bytes()).await;
                    let _ = stream.write_all(body).await;
                });
            }
        });
        port
    }

    #[tokio::test]
    async fn fetch_reads_html_link_icon() {
        const HTML: &[u8] = b"<html><link rel=\"icon\" href=\"/icon.png\"></html>";
        let port = serve(&[("/", "text/html", HTML), ("/icon.png", "image/png", PNG)]).await;
        let data = fetch(&format!("http://127.0.0.1:{port}/")).await.unwrap();
        assert!(data.starts_with("data:image/png;base64,"));
    }

    #[tokio::test]
    async fn fetch_falls_back_to_favicon_ico() {
        let port = serve(&[("/favicon.ico", "image/png", PNG)]).await;
        let data = fetch(&format!("http://127.0.0.1:{port}/missing"))
            .await
            .unwrap();
        assert!(data.starts_with("data:image/png;base64,"));
    }

    #[tokio::test]
    async fn fetch_rejects_javascript() {
        assert!(fetch("javascript:alert(1)").await.is_none());
    }
    #[tokio::test]
    async fn fetch_never_contacts_a_cross_origin_icon() {
        let external = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let external_port = external.local_addr().unwrap().port();
        let service = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = service.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            loop {
                let (mut stream, _) = service.accept().await.unwrap();
                let mut buf = [0; 2048];
                let _ = stream.read(&mut buf).await;
                let body = format!(
                    "<link rel=\"icon\" href=\"http://127.0.0.1:{external_port}/icon.png\">"
                );
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });
        assert!(fetch(&format!("http://127.0.0.1:{port}/")).await.is_none());
        assert!(timeout(Duration::from_millis(100), external.accept())
            .await
            .is_err());
        task.abort();
    }

    #[tokio::test]
    async fn fetch_does_not_follow_redirects() {
        let destination = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let dest_port = destination.local_addr().unwrap().port();
        let redirect = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = redirect.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            loop {
                let (mut stream, _) = redirect.accept().await.unwrap();
                let mut buf = [0; 2048];
                let _ = stream.read(&mut buf).await;
                let response = format!("HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{dest_port}/icon.png\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });
        assert!(fetch(&format!("http://127.0.0.1:{port}/")).await.is_none());
        assert!(timeout(Duration::from_millis(100), destination.accept())
            .await
            .is_err());
        task.abort();
    }
}
