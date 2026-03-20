use std::env;
use std::fs::{self, File};
use std::io::Write as IoWrite;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use futures_util::StreamExt;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

// ============ Types ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub path: String,
    pub port: u16,
    pub interfaces: String,
    pub auth_username: Option<String>,
    pub auth_password: Option<String>,
    pub upload: bool,
    pub mkdir: bool,
    pub media_controls: bool,
    pub color_scheme: String,
    pub title: String,
    // pub hide_icons: bool, // 已禁用
    // pub spa: bool,
    pub compress: String,
    pub hidden: bool,
    pub thumbnails: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            path: String::new(),
            port: 8080,
            interfaces: "0.0.0.0".into(),
            auth_username: None,
            auth_password: None,
            upload: false,
            mkdir: false,
            media_controls: false,
            color_scheme: "squirrel".into(),
            title: "miniserve".into(),
            // hide_icons: false,
            // spa: false,
            compress: "".into(),
            hidden: false,
            thumbnails: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatus {
    pub exists: bool,
    pub version: Option<String>,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub url: Option<String>,       // 主 URL（第一个 IP）
    pub urls: Vec<String>,         // 所有可访问的 URL
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrCodeResponse {
    pub data: String,
}

// ============ State ============

pub struct AppState {
    pub child: Mutex<Option<Child>>,
    pub server_url: Mutex<Option<String>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            child: Mutex::new(None),
            server_url: Mutex::new(None),
        }
    }
}

// ============ Helpers ============

fn get_engine_path() -> PathBuf {
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    let bin_dir = base.join("miniserve-gui").join("bin");
    if env::consts::OS == "windows" {
        bin_dir.join("miniserve.exe")
    } else {
        bin_dir.join("miniserve")
    }
}

fn get_config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("miniserve-gui").join("config.json")
}

fn get_local_ips() -> Vec<String> {
    use std::net::IpAddr;
    let mut ips = Vec::new();

    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        for iface in interfaces {
            // 只取 IPv4，跳过回环地址
            if let IpAddr::V4(ipv4) = iface.addr.ip() {
                if !ipv4.is_loopback() {
                    ips.push(ipv4.to_string());
                }
            }
        }
    }

    // 如果获取失败，回退到 UDP 方式
    if ips.is_empty() {
        if let Some(ip) = get_local_ip_fallback() {
            ips.push(ip);
        }
    }

    ips
}

fn get_local_ip_fallback() -> Option<String> {
    use std::net::UdpSocket;
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let local_addr = socket.local_addr().ok()?;
    Some(local_addr.ip().to_string())
}

fn build_miniserve_args(cfg: &ServerConfig) -> Result<Vec<String>, String> {
    let mut args = vec![];

    // Path must come first (the root directory to serve)
    if !cfg.path.is_empty() {
        args.push(cfg.path.clone());
    }

    // Port
    args.push("-p".into());
    args.push(cfg.port.to_string());

    // Interface
    args.push("-i".into());
    args.push(cfg.interfaces.clone());

    // Auth
    if let (Some(u), Some(p)) = (&cfg.auth_username, &cfg.auth_password) {
        if !u.is_empty() && !p.is_empty() {
            args.push("-a".into());
            args.push(format!("{}:{}", u, p));
        }
    }

    if cfg.upload {
        args.push("-u".into());
    }
    if cfg.mkdir {
        args.push("-M".into());
        args.push(cfg.path.clone());
    }
    if cfg.media_controls {
        args.push("--media-controls".into());
    }
    // Valid values: squirrel, archlinux, zenburn, monokai
    let valid = ["squirrel", "archlinux", "zenburn", "monokai"];
    if valid.contains(&cfg.color_scheme.as_str()) {
        args.push("--color-scheme".into());
        args.push(cfg.color_scheme.clone());
    }
    if !cfg.title.is_empty() {
        args.push("--title".into());
        args.push(cfg.title.clone());
    }
    // if cfg.hide_icons {
    //     args.push("--hide-icons".into());
    // }
    // if cfg.spa {
    //     args.push("--spa --index index.html".into());
    // }
    // if !cfg.compress.is_empty() {
    //     args.push("-c".into());
    //     args.push(cfg.compress.clone());
    // }
    if cfg.hidden {
        args.push("-H".into());
    }
    if cfg.thumbnails {
        args.push("--thumbnails".into());
    }

    Ok(args)
}

// ============ Tauri Commands ============

#[tauri::command]
async fn get_engine_status() -> Result<EngineStatus, String> {
    let path = get_engine_path();
    let exists = path.exists();
    let version = if exists {
        let output = Command::new(&path)
            .arg("--version")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        output.filter(|v| !v.is_empty())
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
async fn download_engine(app_handle: AppHandle) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .user_agent("miniserve-gui-downloader")
        .build()
        .map_err(|e| e.to_string())?;

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

    let release: Release = client
        .get("https://api.github.com/repos/svenstaro/miniserve/releases/latest")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let target_os = env::consts::OS;
    let pattern = match target_os {
        "windows" => "x86_64-pc-windows-msvc",
        "linux" => "x86_64-unknown-linux-musl",
        "macos" => {
            if env::consts::ARCH == "aarch64" {
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

    let mut response = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let total_size = response.content_length().unwrap_or(0);

    let bin_dir = get_engine_path().parent().unwrap().to_path_buf();
    fs::create_dir_all(&bin_dir).map_err(|e| e.to_string())?;

    let tmp_path = bin_dir.join("miniserve.tmp");
    let dest_path = get_engine_path();

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

    info!(
        "Engine downloaded to {} (tag: {})",
        dest_path.display(),
        release.tag_name
    );

    Ok(format!(
        "{} (v{})",
        dest_path.to_string_lossy(),
        release.tag_name
    ))
}

#[tauri::command]
async fn load_config() -> Result<ServerConfig, String> {
    let path = get_config_path();
    if !path.exists() {
        return Ok(ServerConfig::default());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_config(config: ServerConfig) -> Result<(), String> {
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
async fn start_server(
    config: ServerConfig,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<ServerStatus, String> {
    // Kill existing
    {
        let mut child_guard = state.child.lock().map_err(|e| e.to_string())?;
        if let Some(mut c) = child_guard.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    let engine_path = get_engine_path();
    if !engine_path.exists() {
        return Err("引擎未安装，请先下载".into());
    }

    let args = build_miniserve_args(&config)?;
    info!("Starting miniserve with args: {:?}", args);

    let mut child = Command::new(&engine_path)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    // Read stdout and stderr output
    use std::io::{BufRead, BufReader};
    let stdout = child.stdout.take().map(|s| BufReader::new(s));
    let stderr = child.stderr.take().map(|s| BufReader::new(s));

    // Log stdout in background
    let engine_path_for_log = engine_path.clone();
    let args_for_log = args.clone();
    std::thread::spawn(move || {
        if let Some(stdout) = stdout {
            for line in stdout.lines().map_while(Result::ok) {
                log::info!("[miniserve stdout] {}", line);
            }
        }
    });

    // Log stderr in background
    std::thread::spawn(move || {
        if let Some(stderr) = stderr {
            for line in stderr.lines().map_while(Result::ok) {
                log::warn!("[miniserve stderr] {}", line);
            }
        }
    });

    // Wait briefly and check if process is still running
    use std::time::Duration;
    use std::thread;
    thread::sleep(Duration::from_millis(800));

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

    // 生成所有可访问的 URL
    let urls: Vec<String> = if config.interfaces == "0.0.0.0" {
        let ips = get_local_ips();
        if ips.is_empty() {
            vec![format!("http://127.0.0.1:{}", config.port)]
        } else {
            ips.iter().map(|ip| format!("http://{}:{}", ip, config.port)).collect()
        }
    } else {
        vec![format!("http://{}:{}", config.interfaces, config.port)]
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
async fn stop_server(state: State<'_, AppState>, app_handle: AppHandle) -> Result<(), String> {
    let mut child_guard = state.child.lock().map_err(|e| e.to_string())?;
    if let Some(mut c) = child_guard.take() {
        let _ = c.kill();
        let _ = c.wait();
    }
    {
        let mut url_guard = state.server_url.lock().map_err(|e| e.to_string())?;
        *url_guard = None;
    }
    let _ = app_handle.emit("server-stopped", ());
    Ok(())
}

#[tauri::command]
async fn get_server_status(state: State<'_, AppState>) -> Result<ServerStatus, String> {
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
async fn generate_qr(data: String) -> Result<QrCodeResponse, String> {
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

// ============ App Entry ============

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("miniserve-gui starting...");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            get_engine_status,
            download_engine,
            load_config,
            save_config,
            start_server,
            stop_server,
            get_server_status,
            generate_qr,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
