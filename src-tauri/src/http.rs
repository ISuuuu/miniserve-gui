use std::fs;
use std::io::Write;
use std::time::Duration;
use futures_util::StreamExt;
use log::{info, warn};
use tokio::time::timeout;

use crate::state::ServerConfig;
use crate::utils::get_config_path;

/// 规范化代理地址（去空格、去除末尾斜杠，未指定协议头时默认补全 http://）
pub fn normalize_proxy_url(raw: &str) -> String {
    let s = raw.trim().trim_end_matches('/');
    if s.is_empty() {
        return String::new();
    }
    if s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("socks5://")
        || s.starts_with("socks5h://")
    {
        s.to_string()
    } else {
        format!("http://{}", s)
    }
}

/// 解析生效的代理地址。
/// 优先级：1. 用户在设置中自定义的代理；2. 留空时检测系统代理（注册表/平台系统代理）；3. 环境变量 (HTTPS_PROXY等)
pub fn resolve_proxy(custom_proxy: &str) -> Option<String> {
    let custom = custom_proxy.trim();
    if !custom.is_empty() {
        if custom.eq_ignore_ascii_case("direct") || custom.eq_ignore_ascii_case("none") {
            return None;
        }
        return Some(normalize_proxy_url(custom));
    }

    // 留空：自动检测系统代理
    if let Ok(sys) = sysproxy::Sysproxy::get_system_proxy() {
        if sys.enable && !sys.host.is_empty() && sys.port > 0 {
            let host_port = if sys.host.contains(':') && !sys.host.starts_with('[') && !sys.host.starts_with("http") {
                format!("[{}]:{}", sys.host, sys.port)
            } else {
                format!("{}:{}", sys.host, sys.port)
            };
            return Some(normalize_proxy_url(&host_port));
        }
    }

    // 回退：检测常见代理环境变量
    for key in &[
        "HTTPS_PROXY",
        "https_proxy",
        "ALL_PROXY",
        "all_proxy",
        "HTTP_PROXY",
        "http_proxy",
    ] {
        if let Ok(val) = std::env::var(key) {
            let val = val.trim();
            if !val.is_empty() {
                return Some(normalize_proxy_url(val));
            }
        }
    }

    None
}

/// 读取用户配置中设置的代理
pub fn get_configured_proxy() -> String {
    let config_path = get_config_path();
    if config_path.exists() {
        if let Ok(content) = fs::read_to_string(&config_path) {
            if let Ok(config) = serde_json::from_str::<ServerConfig>(&content) {
                return config.proxy;
            }
        }
    }
    String::new()
}

/// 获取当前生效的代理地址（已结合配置、系统代理与环境变量）
pub fn get_effective_proxy() -> Option<String> {
    let custom = get_configured_proxy();
    resolve_proxy(&custom)
}

/// 构建 reqwest 客户端。若指定代理，则配置 Proxy 并设置连接超时；若无代理，则明确配置 no_proxy()。
pub fn build_client(proxy_url: Option<&str>, is_download: bool) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .user_agent("miniserve-gui-downloader")
        .connect_timeout(Duration::from_secs(10));

    if !is_download {
        builder = builder.timeout(Duration::from_secs(15));
    }

    if let Some(proxy_url) = proxy_url {
        let proxy = reqwest::Proxy::all(proxy_url)
            .map_err(|e| format!("INVALID_PROXY:{}: {}", proxy_url, e))?;
        builder = builder.proxy(proxy);
    } else {
        builder = builder.no_proxy();
    }

    builder.build().map_err(|e| e.to_string())
}

/// Stream body chunks with a per-chunk inactivity timeout (30 seconds)
/// to avoid hanging indefinitely if the remote server stalls.
pub async fn stream_response_to_file<F>(
    response: reqwest::Response,
    mut file: fs::File,
    mut on_chunk: F,
) -> Result<u64, String>
where
    F: FnMut(&[u8], u64, u64),
{
    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    let chunk_timeout = Duration::from_secs(30);

    loop {
        let next_item = match timeout(chunk_timeout, stream.next()).await {
            Ok(Some(item)) => item,
            Ok(None) => break, // EOF
            Err(_) => {
                return Err("DOWNLOAD_TIMEOUT:下载数据流读取超时 (30s 无响应)".to_string());
            }
        };

        let chunk = next_item.map_err(|e| format!("DOWNLOAD_CHUNK_FAILED:{}", e))?;
        file.write_all(&chunk)
            .map_err(|e| format!("FILE_WRITE_FAILED:{}", e))?;
        downloaded += chunk.len() as u64;

        on_chunk(&chunk, downloaded, total_size);
    }

    file.flush()
        .map_err(|e| format!("FILE_FLUSH_FAILED:{}", e))?;
    Ok(downloaded)
}

/// 发送请求并获取响应头
pub async fn send_request(
    client: &reqwest::Client,
    url: &str,
    header_timeout: Option<Duration>,
) -> Result<reqwest::Response, String> {
    let fut = client.get(url).send();
    match header_timeout {
        Some(d) => timeout(d, fut)
            .await
            .map_err(|_| "HEADER_TIMEOUT:等待响应头超时 (30s)".to_string())?
            .map_err(|e| format!("NETWORK_ERROR:{}", e)),
        None => fut.await.map_err(|e| format!("NETWORK_ERROR:{}", e)),
    }
}

/// 优先使用代理通道，失败（网络错误、超时或服务端异常）自动回退到直连通道
pub async fn fetch_url_with_fallback(
    url: &str,
    proxy_url: Option<&str>,
    is_download: bool,
    header_timeout: Option<Duration>,
) -> Result<reqwest::Response, String> {
    if let Some(proxy) = proxy_url {
        info!("首选代理通道 ({}) 请求: {}", proxy, url);
        match build_client(Some(proxy), is_download) {
            Ok(proxy_client) => {
                match send_request(&proxy_client, url, header_timeout).await {
                    Ok(resp) if resp.status().is_success() => return Ok(resp),
                    res => {
                        let err_desc = match &res {
                            Ok(r) => format!("HTTP {}", r.status()),
                            Err(e) => e.clone(),
                        };
                        warn!("代理通道请求失败 ({})，回退到直连: {}", err_desc, url);
                    }
                }
            }
            Err(e) => {
                warn!("构建代理客户端失败 ({})，回退到直连: {}", e, url);
            }
        }
    }

    let direct_client = build_client(None, is_download)?;
    send_request(&direct_client, url, header_timeout).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_proxy_url() {
        assert_eq!(normalize_proxy_url("127.0.0.1:7897"), "http://127.0.0.1:7897");
        assert_eq!(normalize_proxy_url("127.0.0.1:7897/"), "http://127.0.0.1:7897");
        assert_eq!(normalize_proxy_url("http://127.0.0.1:7897"), "http://127.0.0.1:7897");
        assert_eq!(normalize_proxy_url("http://127.0.0.1:7897/"), "http://127.0.0.1:7897");
        assert_eq!(normalize_proxy_url("https://proxy.example.com:8443/"), "https://proxy.example.com:8443");
        assert_eq!(normalize_proxy_url("socks5://127.0.0.1:1080"), "socks5://127.0.0.1:1080");
        assert_eq!(normalize_proxy_url("socks5h://127.0.0.1:1080/"), "socks5h://127.0.0.1:1080");
        assert_eq!(normalize_proxy_url("   "), "");
    }

    #[test]
    fn test_resolve_proxy_custom() {
        assert_eq!(resolve_proxy("127.0.0.1:7897"), Some("http://127.0.0.1:7897".into()));
        assert_eq!(resolve_proxy("http://127.0.0.1:7897"), Some("http://127.0.0.1:7897".into()));
        assert_eq!(resolve_proxy("socks5://127.0.0.1:7897"), Some("socks5://127.0.0.1:7897".into()));
        assert_eq!(resolve_proxy("direct"), None);
        assert_eq!(resolve_proxy("DIRECT"), None);
        assert_eq!(resolve_proxy("none"), None);
    }
}
