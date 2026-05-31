use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write as IoWrite};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use futures_util::StreamExt;
use log::info;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::state::{AppState, EngineStatus, QrCodeResponse, ServerConfig, ServerStatus};
use crate::utils::{build_miniserve_args, get_config_path, get_engine_path, get_local_ips, validate_config};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// ============ Shared Helpers ============

/// Run `miniserve --version` and return the raw version string (e.g. "miniserve 0.27.0").
fn get_engine_version(path: &std::path::Path) -> Option<String> {
    let mut cmd = Command::new(path);
    cmd.arg("--version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Prepend proxy prefix to a URL. Returns None if proxy is empty.
fn apply_proxy(proxy_prefix: &str, url: &str) -> Option<String> {
    if proxy_prefix.is_empty() {
        None
    } else {
        Some(format!("{}{}", proxy_prefix, url))
    }
}

/// Build a reqwest::Client with proxy support.
fn build_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("miniserve-gui-downloader")
        .build()
        .map_err(|e| e.to_string())
}

/// Fetch with proxy fallback: try direct first, then proxy if it fails.
async fn fetch_with_proxy(
    client: &reqwest::Client,
    direct_url: &str,
    proxy_url: Option<&str>,
) -> Result<reqwest::Response, String> {
    match client.get(direct_url).send().await {
        Ok(resp) if resp.status().is_success() => Ok(resp),
        _ => {
            if let Some(proxy) = proxy_url {
                info!("直连失败，尝试使用代理: {}", proxy);
                client.get(proxy).send().await.map_err(|e| format!("代理也无法访问: {}", e))
            } else {
                Err("直连失败且未配置代理".into())
            }
        }
    }
}

// ============ Tauri Commands ============

#[tauri::command]
pub async fn get_engine_status() -> Result<EngineStatus, String> {
    let path = get_engine_path();
    let exists = path.exists();
    let version = if exists {
        get_engine_version(&path)
    } else {
        None
    };

    Ok(EngineStatus {
        exists,
        version,
        path: path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn download_engine(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    {
        let child_guard = state.child.lock().map_err(|e| e.to_string())?;
        if child_guard.is_some() {
            return Err("服务正在运行中，请先停止服务再更新引擎".into());
        }
    }

    #[derive(Deserialize, Debug)]
    struct Release {
        tag_name: String,
        assets: Vec<Asset>,
    }
    #[derive(Deserialize, Debug)]
    struct Asset {
        name: String,
        browser_download_url: String,
    }

    let client = build_http_client()?;
    let api_url = "https://api.github.com/repos/svenstaro/miniserve/releases/latest";
    let proxy_prefix = get_proxy_prefix(&app_handle).unwrap_or_default();
    if !proxy_prefix.is_empty() && !proxy_prefix.ends_with('/') {
        return Err("代理 URL 必须以 / 结尾，例如 https://proxy.example.com/".into());
    }
    let proxy_api_url = apply_proxy(&proxy_prefix, api_url);

    // Fetch latest release
    let response = fetch_with_proxy(&client, api_url, proxy_api_url.as_deref()).await?;
    if !response.status().is_success() {
        let err_text = response.text().await.unwrap_or_default();
        #[derive(Deserialize, Debug)]
        struct GithubError { message: String }
        let msg = serde_json::from_str::<GithubError>(&err_text)
            .map(|e| e.message)
            .unwrap_or(err_text);
        return Err(format!("获取版本失败: {}", msg));
    }

    let release: Release = response.json().await.map_err(|e| e.to_string())?;

    // Check if already up to date
    let dest_path = get_engine_path();
    if let Some(current_ver) = dest_path.exists().then(|| get_engine_version(&dest_path)).flatten() {
        let current_num = current_ver.replace("miniserve ", "");
        let latest_ver = release.tag_name.trim_start_matches('v');
        if current_num == latest_ver {
            return Ok(format!("已是最新版本 (v{})", latest_ver));
        }
    }

    // Find matching asset
    let target_os = std::env::consts::OS;
    let pattern = match target_os {
        "windows" => "x86_64-pc-windows-msvc",
        "linux" => "x86_64-unknown-linux-musl",
        "macos" => {
            if std::env::consts::ARCH == "aarch64" {
                "aarch64-apple-darwin"
            } else {
                "x86_64-apple-darwin"
            }
        }
        _ => return Err("Unsupported OS".into()),
    };

    let asset = release
        .assets
        .iter()
        .find(|a| a.name.contains(pattern))
        .ok_or("No matching binary found")?;

    // Download binary with proxy fallback
    let proxy_download_url = apply_proxy(&proxy_prefix, &asset.browser_download_url);
    let response = fetch_with_proxy(&client, &asset.browser_download_url, proxy_download_url.as_deref()).await?;

    let total_size = response.content_length().unwrap_or(0);
    let bin_dir = get_engine_path().parent().unwrap().to_path_buf();
    fs::create_dir_all(&bin_dir).map_err(|e| e.to_string())?;

    let tmp_path = bin_dir.join("miniserve.tmp");
    let mut file = File::create(&tmp_path).map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;

    let mut stream = response.bytes_stream();
    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| e.to_string())?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;
        let pct = if total_size > 0 {
            (downloaded as f64 / total_size as f64) * 100.0
        } else {
            0.0
        };
        let _ = app_handle.emit("download-progress", pct);
    }
    drop(file);

    fs::rename(&tmp_path, &dest_path).map_err(|e| e.to_string())?;

    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&dest_path).map_err(|e| e.to_string())?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest_path, perms).map_err(|e| e.to_string())?;
    }

    info!("Engine downloaded to {} (tag: {})", dest_path.display(), release.tag_name);
    Ok(format!("{} (v{})", dest_path.to_string_lossy(), release.tag_name))
}

#[tauri::command]
pub async fn load_config() -> Result<ServerConfig, String> {
    let path = get_config_path();
    if !path.exists() {
        return Ok(ServerConfig::default());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_config(config: ServerConfig) -> Result<(), String> {
    let path = get_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())?;
    info!("Config saved to {}", path.display());
    Ok(())
}

#[tauri::command]
pub async fn start_server(
    config: ServerConfig,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<ServerStatus, String> {
    // 防止并发启动
    if state.starting.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return Err("服务正在启动中，请稍候".into());
    }

    // 使用 drop guard 确保 starting 标志在函数退出时重置
    struct StartingGuard<'a>(&'a AtomicBool);
    impl Drop for StartingGuard<'_> {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }
    let _guard = StartingGuard(&state.starting);

    // 验证配置
    validate_config(&config)?;

    // Kill existing process and clean up job object
    state.kill_child()?;

    let engine_path = get_engine_path();
    if !engine_path.exists() {
        return Err("引擎未安装，请先下载".into());
    }

    let args = build_miniserve_args(&config)?;
    info!("Starting miniserve with args: {:?}", args);

    let mut child = Command::new(&engine_path);
    child
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        child.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = child.spawn().map_err(|e| e.to_string())?;

    // Windows: Create Job Object and assign child process
    // This ensures child processes are killed when parent exits unexpectedly
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        let job = crate::job_object::create_kill_on_close_job()?;
        let process_handle = child.as_raw_handle();
        crate::job_object::assign_process_to_job(job, process_handle as *mut _)?;
        if let Ok(mut job_guard) = state.job_handle.lock() {
            *job_guard = Some(job);
        }
        log::info!("Child process assigned to job object (kill-on-close)");
    }

    // Read stdout and stderr output
    let stdout = child.stdout.take().map(|s| BufReader::new(s));
    let stderr = child.stderr.take().map(|s| BufReader::new(s));

    // For capturing random route from stdout
    let (tx_route, rx_route) = std::sync::mpsc::channel();

    // Log stdout in background and capture random route, emit to frontend
    let app_handle_clone = app_handle.clone();
    let engine_path_for_log = engine_path.clone();
    let args_for_log = args.clone();
    let capture_route = config.random_route;
    let target_port = config.port;
    thread::spawn(move || {
        if let Some(stdout) = stdout {
            for line in stdout.lines().map_while(Result::ok) {
                let trimmed = line.trim();
                log::info!("{}", trimmed);
                let _ = app_handle_clone.emit("server-log", trimmed);
                // Try to capture random route from output like:
                // "http://192.168.6.133:8080/857613"
                if capture_route {
                    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                        // Extract path part after port
                        let port_str = format!(":{}", target_port);
                        if let Some(port_pos) = trimmed.find(&port_str) {
                            let path_part = trimmed[port_pos + port_str.len()..].trim_end_matches('/');
                            if !path_part.is_empty() {
                                let _ = tx_route.send(path_part.to_string());
                            }
                        }
                    }
                }
            }
        }
    });

    // Log stderr in background, emit to frontend
    let app_handle_clone2 = app_handle.clone();
    thread::spawn(move || {
        if let Some(stderr) = stderr {
            for line in stderr.lines().map_while(Result::ok) {
                log::warn!("{}", line);
                let _ = app_handle_clone2.emit("server-log", line.trim());
            }
        }
    });

    // Wait briefly and check if process is still running
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;

    if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
        // Re-construct command string for error message
        let cmd_str = std::iter::once(engine_path_for_log.to_string_lossy().to_string())
            .chain(args_for_log)
            .collect::<Vec<_>>()
            .join(" ");
        return Err(format!(
            "miniserve 启动失败 (exit code: {:?})\n命令: {}\n请查看日志获取详细信息",
            status.code(),
            cmd_str
        ));
    }

    let pid = child.id();

    // 尝试获取随机路由（如果有的话）
    let random_route = if config.random_route {
        let route = rx_route.recv_timeout(std::time::Duration::from_millis(500)).ok();
        log::info!("[debug] received route: {:?}", route);
        route
    } else {
        None
    };

    // 生成所有可访问的 URL
    let route_suffix = random_route.clone().unwrap_or_default();
    log::info!("[debug] final route_suffix: {}", route_suffix);
    let urls: Vec<String> = if config.interfaces == "0.0.0.0" || config.interfaces == "::" {
        let ips = get_local_ips();
        if ips.is_empty() {
            vec![format!("http://127.0.0.1:{}{}", config.port, route_suffix)]
        } else {
            ips.iter().map(|ip| format!("http://{}:{}{}", ip, config.port, route_suffix)).collect()
        }
    } else if config.interfaces.contains(':') && !config.interfaces.starts_with('[') {
        // Specific IPv6 address
        vec![format!("http://[{}]:{}{}", config.interfaces, config.port, route_suffix)]
    } else {
        // Specific IPv4 address
        vec![format!("http://{}:{}{}", config.interfaces, config.port, route_suffix)]
    };

    let url = urls.first().cloned();

    {
        let mut child_guard = state.child.lock().map_err(|e| e.to_string())?;
        *child_guard = Some(child);
    }
    {
        let mut url_guard = state.server_url.lock().map_err(|e| e.to_string())?;
        *url_guard = url.clone();
    }

    let _ = app_handle.emit("server-started", &urls);

    Ok(ServerStatus {
        running: true,
        pid: Some(pid),
        url,
        urls,
        port: Some(config.port),
    })
}

#[tauri::command]
pub async fn stop_server(state: State<'_, AppState>, app_handle: AppHandle) -> Result<(), String> {
    state.kill_child()?;
    {
        let mut url_guard = state.server_url.lock().map_err(|e| e.to_string())?;
        *url_guard = None;
    }
    let _ = app_handle.emit("server-stopped", ());
    Ok(())
}

#[tauri::command]
pub async fn get_server_status(state: State<'_, AppState>) -> Result<ServerStatus, String> {
    let child_guard = state.child.lock().map_err(|e| e.to_string())?;
    let url_guard = state.server_url.lock().map_err(|e| e.to_string())?;

    let running = child_guard.is_some();
    let pid = child_guard.as_ref().map(|c| c.id());
    let url = url_guard.clone();
    let urls: Vec<String> = url.iter().cloned().collect();

    Ok(ServerStatus {
        running,
        pid,
        url,
        urls,
        port: None,
    })
}

#[tauri::command]
pub async fn generate_qr(data: String) -> Result<QrCodeResponse, String> {
    use qrcode::QrCode;
    use image::Luma;

    let code = QrCode::new(data.as_bytes()).map_err(|e| e.to_string())?;
    let image = code.render::<Luma<u8>>().build();

    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    image
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;

    let base64_data =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &buf);

    Ok(QrCodeResponse {
        data: format!("data:image/png;base64,{}", base64_data),
    })
}

#[tauri::command]
pub fn get_install_dir() -> Result<String, String> {
    let dir = std::env::current_exe()
        .and_then(|p| {
            p.parent()
                .map(|p| p.to_path_buf())
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "No parent dir"))
        })
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())?;

    // Return the original long path directly without short path conversion
    // NSIS /D= parameter supports long paths with spaces
    Ok(dir)
}

#[derive(Serialize)]
pub struct UpdaterConfig {
    pub endpoints: Vec<String>,
    pub proxy: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct UpdaterPluginConfig {
    pub endpoints: Vec<String>,
    #[serde(default)]
    pub proxy: Option<String>,
}

/// Get the GitHub proxy prefix. User config takes priority over tauri.conf.json.
pub fn get_proxy_prefix(app_handle: &AppHandle) -> Option<String> {
    // 1. Try user config first
    let config_path = get_config_path();
    if config_path.exists() {
        if let Ok(content) = fs::read_to_string(&config_path) {
            if let Ok(config) = serde_json::from_str::<ServerConfig>(&content) {
                if !config.github_proxy.is_empty() {
                    return Some(config.github_proxy);
                }
            }
        }
    }
    // 2. Fall back to tauri.conf.json updater plugin config
    let plugins = &app_handle.config().plugins.0;
    let updater_value = plugins.get("updater")?;
    let config: UpdaterPluginConfig = serde_json::from_value(updater_value.clone()).ok()?;
    config.proxy
}

/// 获取更新器公钥（来自 tauri.conf.json plugins.updater.pubkey）
fn get_updater_pubkey(app_handle: &AppHandle) -> Result<String, String> {
    let plugins = &app_handle.config().plugins.0;
    let updater_value = plugins.get("updater").ok_or("updater config not found")?;
    updater_value
        .get("pubkey")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or("updater pubkey not found in config".into())
}

/// 验证下载文件的 minisign 签名
fn verify_signature(data: &[u8], signature_b64: &str, pubkey_b64: &str) -> Result<(), String> {
    use minisign_verify::{PublicKey, Signature};
    let public_key = PublicKey::decode(pubkey_b64)
        .map_err(|e| format!("解析公钥失败: {}", e))?;
    let signature = Signature::decode(signature_b64)
        .map_err(|e| format!("解析签名失败: {}", e))?;
    public_key.verify(data, &signature, false)
        .map_err(|_| "签名验证失败：文件可能已被篡改".into())
}

/// 以 root 权限替换 AppImage。shell 片段是固定字符串，路径只通过 argv 传入，
/// 避免拼接用户可控路径，也不在 /tmp 落可被替换的脚本文件。
#[cfg(target_os = "linux")]
fn run_pkexec_appimage_replace(
    target_path: &str,
    source_path: &str,
    final_path: &str,
) -> Result<std::process::Output, String> {
    std::process::Command::new("pkexec")
        .args([
            "sh",
            "-c",
            "set -e; rm -f -- \"$1\"; cp -- \"$2\" \"$3\"",
            "miniserve-gui-update",
            target_path,
            source_path,
            final_path,
        ])
        .output()
        .map_err(|e| format!("pkexec 执行失败: {}", e))
}

#[tauri::command]
pub fn get_updater_config(app_handle: AppHandle) -> Result<UpdaterConfig, String> {
    let plugins = &app_handle.config().plugins.0;
    let updater_value = plugins.get("updater").ok_or("updater config not found")?;
    let config: UpdaterPluginConfig = serde_json::from_value(updater_value.clone())
        .map_err(|e| format!("failed to parse updater config: {}", e))?;

    Ok(UpdaterConfig {
        endpoints: config.endpoints,
        proxy: config.proxy,
    })
}

/// Fetch update manifest with proxy fallback (moved from frontend to avoid CSP issues).
#[tauri::command]
pub async fn fetch_update_manifest(
    app_handle: AppHandle,
    url: String,
) -> Result<serde_json::Value, String> {
    let client = build_http_client()?;
    let proxy_prefix = get_proxy_prefix(&app_handle).unwrap_or_default();
    let proxy_url = if !proxy_prefix.is_empty() {
        apply_proxy(&proxy_prefix, &url)
    } else {
        None
    };

    let response = fetch_with_proxy(&client, &url, proxy_url.as_deref()).await?;
    if !response.status().is_success() {
        return Err(format!("更新清单响应异常: {}", response.status()));
    }
    response.json().await.map_err(|e| format!("解析更新清单失败: {}", e))
}

#[tauri::command]
pub async fn download_and_install_update(
    app_handle: AppHandle,
    url: String,
    signature: String,
    version: String,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        // 如果当前不是 AppImage 环境，说明是通过 deb 或其他方式安装的
        // 直接返回错误，让前端引导用户去 Release 页面下载
        if std::env::var("APPIMAGE").is_err() {
            return Err("由于您使用的是非便携版本，请前往 Github Release 页面下载最新的安装包进行更新。".into());
        }
    }

    info!("开始下载更新 v{}: {}", version, url);
    let client = build_http_client()?;

    let proxy_prefix = get_proxy_prefix(&app_handle).unwrap_or_default();
    let download_url = if !proxy_prefix.is_empty() && url.contains("github.com") {
        apply_proxy(&proxy_prefix, &url).unwrap_or(url.clone())
    } else {
        url.clone()
    };

    info!("下载更新: {}", download_url);

    let response = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("下载失败: HTTP {}", response.status()));
    }

    let bytes = response.bytes().await.map_err(|e| e.to_string())?;

    // 验证签名
    if !signature.is_empty() {
        match get_updater_pubkey(&app_handle) {
            Ok(pubkey) => {
                verify_signature(&bytes, &signature, &pubkey)?;
                info!("签名验证通过");
            }
            Err(e) => {
                return Err(format!("无法获取公钥进行签名验证: {}", e));
            }
        }
    } else {
        return Err("更新清单未提供签名，拒绝安装".into());
    }

    // 使用随机临时文件名，防止符号链接攻击
    let ext = std::path::Path::new(url.split('/').last().unwrap_or("update"))
        .extension()
        .unwrap_or(std::ffi::OsStr::new("exe"));
    let mut temp_file = tempfile::Builder::new()
        .prefix("miniserve-update-")
        .suffix(&format!(".{}", ext.to_string_lossy()))
        .tempfile_in(std::env::temp_dir())
        .map_err(|e| format!("创建临时文件失败: {}", e))?;
    use std::io::Write;
    temp_file.write_all(&bytes).map_err(|e| e.to_string())?;
    let temp_path = temp_file.into_temp_path();
    info!("更新已下载到: {:?}", temp_path);

    #[cfg(windows)]
    {
        use std::process::Command;
        let install_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));

        let mut cmd = Command::new(&temp_path);
        cmd.arg("/S").arg("/R");
        if let Some(dir) = install_dir {
            cmd.arg(format!("/D={}", dir.display()));
        }

        cmd.spawn().map_err(|e| format!("启动安装程序失败: {}", e))?;
        info!("安装程序已启动，正在退出当前进程以释放文件锁");
        app_handle.exit(0);
    }

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;

        let source_path = temp_path.to_string_lossy().to_string();

        let (output, relaunch_path) = if source_path.ends_with(".deb") {
            info!("检测到 deb 文件，使用 dpkg 安装...");
            // 直接传参给 pkexec，不经过 sh -c，避免 shell 注入
            let output = std::process::Command::new("pkexec")
                .args(["dpkg", "-i", &source_path])
                .output()
                .map_err(|e| format!("pkexec 执行失败: {}", e))?;
            (output, None)
        } else {
            info!("执行 AppImage/二进制文件 替换...");
            // 设置可执行权限
            let mut perms = fs::metadata(&temp_path).map_err(|e| e.to_string())?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&temp_path, perms).map_err(|e| e.to_string())?;

            // 获取真实目标路径：如果是 AppImage 运行，必须通过环境变量获取真正的外部文件路径
            let target_path = if let Ok(appimage_path) = std::env::var("APPIMAGE") {
                appimage_path
            } else {
                std::env::current_exe().map_err(|e| e.to_string())?.to_string_lossy().to_string()
            };

            let target_path_buf = std::path::PathBuf::from(&target_path);
            let target_dir = target_path_buf.parent().unwrap();
            // 使用 Path::file_name() 提取文件名，防止路径穿越
            let raw_name = url
                .split('/')
                .last()
                .filter(|name| name.ends_with(".AppImage"))
                .ok_or("无法从更新 URL 获取 AppImage 文件名")?;
            let final_name = std::path::Path::new(raw_name)
                .file_name()
                .ok_or("AppImage 文件名无效")?;
            let final_path = target_dir.join(final_name).to_string_lossy().to_string();

            let output = run_pkexec_appimage_replace(&target_path, &source_path, &final_path)?;
            (output, Some(final_path))
        };

        if output.status.success() {
            info!("更新安装成功，重启应用");
            if let Some(path) = relaunch_path {
                std::process::Command::new(&path)
                    .spawn()
                    .map_err(|e| format!("启动新版 AppImage 失败: {}", e))?;
                app_handle.exit(0);
            } else {
                app_handle.restart();
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::error!("pkexec 失败: {}", stderr);
            return Err(format!("更新失败: {}", stderr.trim()));
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let target_path = current_exe.to_string_lossy().to_string();

        // macOS: 替换并重启
        fs::copy(&temp_path, &target_path).map_err(|e| format!("替换失败: {}", e))?;

        // 设置可执行权限
        Command::new("chmod")
            .args(["+x", &target_path])
            .status()
            .ok();

        info!("更新安装成功，重启应用");
        app_handle.restart();
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    Ok(())
}

#[tauri::command]
pub fn get_package_type() -> String {
    #[cfg(target_os = "linux")]
    {
        if std::env::var("APPIMAGE").is_ok() {
            return "appimage".to_string();
        }
        // Linux 下如果不是 AppImage，默认视为 deb/已安装版本
        return "deb".to_string();
    }

    #[cfg(target_os = "windows")]
    {
        if crate::utils::is_portable() {
            return "portable".to_string();
        }
        return "installer".to_string();
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    "unknown".to_string()
}

#[tauri::command]
pub fn show_window_command(app_handle: AppHandle) -> Result<(), String> {
    crate::show_window(&app_handle);
    Ok(())
}
