use std::env;
use std::fs::{self, File};
use std::io::Write as IoWrite;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use futures_util::StreamExt;
use log::info;
use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, Manager, State,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

// ============ Windows Job Object (防止子进程成为孤儿) ============

#[cfg(windows)]
mod job_object {
    use std::ffi::c_void;

    type HANDLE = *mut c_void;
    type BOOL = i32;

    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;

    /// JOBOBJECT_BASIC_LIMIT_INFORMATION on x64
    #[repr(C)]
    struct BasicLimitInfo {
        per_process_user_time_limit: i64,   // LARGE_INTEGER
        per_job_user_time_limit: i64,       // LARGE_INTEGER
        limit_flags: u32,                   // DWORD
        _pad1: u32,                         // padding for SIZE_T alignment
        minimum_working_set_size: usize,    // SIZE_T
        maximum_working_set_size: usize,    // SIZE_T
        active_process_limit: u32,          // DWORD
        _pad2: u32,                         // padding for ULONG_PTR alignment
        affinity: usize,                    // ULONG_PTR
        priority_class: u32,                // DWORD
        scheduling_class: u32,              // DWORD
    }

    /// JOBOBJECT_EXTENDED_LIMIT_INFORMATION on x64 (144 bytes)
    #[repr(C)]
    struct ExtendedLimitInfo {
        basic: BasicLimitInfo,              // 64 bytes
        io_counters: [u8; 48],              // IO_COUNTERS
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(lp_job_attributes: *const c_void, lp_name: *const u16) -> HANDLE;
        fn SetInformationJobObject(
            h_job: HANDLE,
            job_object_info_class: i32,
            lp_job_object_info: *const c_void,
            cb_job_object_info_length: u32,
        ) -> BOOL;
        fn AssignProcessToJobObject(h_job: HANDLE, h_process: HANDLE) -> BOOL;
        fn CloseHandle(h_object: HANDLE) -> BOOL;
    }

    /// 创建一个 Job Object，设置 KILL_ON_JOB_CLOSE 标志
    /// 当最后一个句柄关闭时，所有子进程会被自动杀死
    pub fn create_kill_on_close_job() -> Result<HANDLE, String> {
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(format!(
                    "CreateJobObjectW failed: {}",
                    std::io::Error::last_os_error()
                ));
            }

            let mut info: ExtendedLimitInfo = std::mem::zeroed();
            info.basic.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            // JobObjectExtendedLimitInformation = 9
            let ret = SetInformationJobObject(
                job,
                9,
                &info as *const _ as *const c_void,
                std::mem::size_of::<ExtendedLimitInfo>() as u32,
            );
            if ret == 0 {
                let err = std::io::Error::last_os_error();
                log::error!(
                    "[JobObject] SetInformationJobObject failed: {} (size={})",
                    err,
                    std::mem::size_of::<ExtendedLimitInfo>()
                );
                let _ = CloseHandle(job);
                return Err(format!("SetInformationJobObject failed: {}", err));
            }

            log::info!(
                "[JobObject] Created OK (size={})",
                std::mem::size_of::<ExtendedLimitInfo>()
            );
            Ok(job)
        }
    }

    /// 将子进程分配到 Job Object
    pub fn assign_process_to_job(job: HANDLE, process_handle: HANDLE) -> Result<(), String> {
        unsafe {
            let ret = AssignProcessToJobObject(job, process_handle);
            if ret == 0 {
                return Err(format!(
                    "AssignProcessToJobObject failed: {}",
                    std::io::Error::last_os_error()
                ));
            }
            Ok(())
        }
    }

    /// 关闭 Job Object 句柄（会触发 KILL_ON_JOB_CLOSE）
    pub fn close_job(job: HANDLE) {
        unsafe {
            let _ = CloseHandle(job);
        }
    }
}

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
    #[serde(default)]
    pub random_route: bool,
    #[serde(default)]
    pub readme: bool,
    #[serde(default)]
    pub download: bool,
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
            random_route: false,
            readme: false,
            download: false,
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
    #[cfg(windows)]
    pub job_handle: Mutex<Option<*mut std::ffi::c_void>>,
}

// SAFETY: job_handle is protected by Mutex, and Win32 handles are safe to send between threads
#[cfg(windows)]
unsafe impl Send for AppState {}
#[cfg(windows)]
unsafe impl Sync for AppState {}

impl Default for AppState {
    fn default() -> Self {
        Self {
            child: Mutex::new(None),
            server_url: Mutex::new(None),
            #[cfg(windows)]
            job_handle: Mutex::new(None),
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
    let mut ips_v4 = Vec::new();
    let mut ips_v6 = Vec::new();

    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        for iface in interfaces {
            match iface.addr.ip() {
                IpAddr::V4(ipv4) => {
                    if !ipv4.is_loopback() {
                        ips_v4.push(ipv4.to_string());
                    }
                }
                IpAddr::V6(ipv6) => {
                    if !ipv6.is_loopback() {
                        let segs = ipv6.segments();
                        let first = segs[0];
                        // Filter out internal IPv6:
                        // fe80::/10 link-local
                        let is_link_local = (first & 0xffc0) == 0xfe80;
                        // fc00::/7 unique local (ULA - internal network)
                        let is_ula = (first & 0xfe00) == 0xfc00;
                        // fec0::/10 site local (deprecated)
                        let is_site_local = (first & 0xffc0) == 0xfec0;

                        if !is_link_local && !is_ula && !is_site_local {
                            ips_v6.push(format!("[{}]", ipv6.to_string()));
                        }
                    }
                }
            }
        }
    }

    // 如果获取失败，回退到 UDP 方式
    if ips_v4.is_empty() {
        if let Some(ip) = get_local_ip_fallback() {
            ips_v4.push(ip);
        }
    }

    ips_v4.extend(ips_v6);
    ips_v4
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
    if cfg.interfaces == "::" {
        args.push("-i".into());
        args.push("0.0.0.0".into());
        args.push("-i".into());
        args.push("::".into());
    } else {
        args.push("-i".into());
        args.push(cfg.interfaces.clone());
    }

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
        args.push("-u".into());
        args.push("-U".into());
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
    if cfg.random_route {
        args.push("--random-route".into());
    }
    if cfg.readme {
        args.push("--readme".into());
    }
    if cfg.download {
        args.push("-z".into());
    }

    // Verbose mode to show all details in logs
    args.push("-v".into());

    Ok(args)
}

// ============ Tauri Commands ============

#[tauri::command]
async fn get_engine_status() -> Result<EngineStatus, String> {
    let path = get_engine_path();
    let exists = path.exists();
    let version = if exists {
        let mut cmd = Command::new(&path);
        cmd.arg("--version");
        
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        
        let output = cmd
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
async fn download_engine(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    {
        let child_guard = state.child.lock().map_err(|e| e.to_string())?;
        if child_guard.is_some() {
            return Err("服务正在运行中，请先停止服务再更新引擎".into());
        }
    }
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

    let original_url = "https://api.github.com/repos/svenstaro/miniserve/releases/latest";
    let proxy_prefix = get_proxy_prefix(&app_handle).unwrap_or_default();
    let proxy_url = if proxy_prefix.is_empty() {
        String::new()
    } else {
        format!("{}{}", proxy_prefix, original_url)
    };

    let response = match client.get(original_url).send().await {
        Ok(resp) if resp.status().is_success() => resp,
        _ if !proxy_url.is_empty() => {
            info!("直连失败，尝试使用代理: {}", proxy_url);
            client.get(&proxy_url).send().await.map_err(|e| format!("代理也无法访问: {}", e))?
        }
        _ => {
            return Err("直连失败且未配置代理".into());
        }
    };

    if !response.status().is_success() {
        let err_text = response.text().await.unwrap_or_default();
        #[derive(Deserialize, Debug)]
        struct GithubError {
            message: String,
        }
        let msg = if let Ok(gh_err) = serde_json::from_str::<GithubError>(&err_text) {
            gh_err.message
        } else {
            err_text
        };
        return Err(format!("获取版本失败: {}", msg));
    }

    let release: Release = response
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let dest_path = get_engine_path();
    if dest_path.exists() {
        let mut cmd = Command::new(&dest_path);
        cmd.arg("--version");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        if let Ok(output) = cmd.output() {
            let version_str = String::from_utf8_lossy(&output.stdout);
            let current_ver = version_str.trim().replace("miniserve ", "");
            let latest_ver = release.tag_name.trim_start_matches('v');
            if current_ver == latest_ver {
                return Ok(format!("已是最新版本 (v{})", latest_ver));
            }
        }
    }

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

    let download_url = &asset.browser_download_url;
    let proxy_download_url = if proxy_prefix.is_empty() {
        String::new()
    } else {
        format!("{}{}", proxy_prefix, download_url)
    };

    let response = match client.get(download_url).send().await {
        Ok(resp) if resp.status().is_success() => resp,
        _ if !proxy_download_url.is_empty() => {
            info!("直连下载失败，尝试使用代理: {}", proxy_download_url);
            client.get(&proxy_download_url).send().await.map_err(|e| format!("代理下载失败: {}", e))?
        }
        _ => {
            return Err("直连下载失败且未配置代理".into());
        }
    };

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
    // Kill existing and clean up job object
    {
        let mut child_guard = state.child.lock().map_err(|e| e.to_string())?;
        if let Some(mut c) = child_guard.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
    #[cfg(windows)]
    {
        if let Ok(mut job_guard) = state.job_handle.lock() {
            if let Some(job) = job_guard.take() {
                job_object::close_job(job);
            }
        }
    }

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
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        child.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = child.spawn().map_err(|e| e.to_string())?;

    // Windows: Create Job Object and assign child process
    // This ensures child processes are killed when parent exits unexpectedly
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        let job = job_object::create_kill_on_close_job()?;
        let process_handle = child.as_raw_handle();
        job_object::assign_process_to_job(job, process_handle as *mut _)?;
        if let Ok(mut job_guard) = state.job_handle.lock() {
            *job_guard = Some(job);
        }
        log::info!("Child process assigned to job object (kill-on-close)");
    }

    // Read stdout and stderr output
    use std::io::{BufRead, BufReader};
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
    std::thread::spawn(move || {
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
    std::thread::spawn(move || {
        if let Some(stderr) = stderr {
            for line in stderr.lines().map_while(Result::ok) {
                log::warn!("{}", line);
                let _ = app_handle_clone2.emit("server-log", line.trim());
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
    #[cfg(windows)]
    {
        if let Ok(mut job_guard) = state.job_handle.lock() {
            if let Some(job) = job_guard.take() {
                job_object::close_job(job);
            }
        }
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

#[tauri::command]
fn get_install_dir() -> Result<String, String> {
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
struct UpdaterConfig {
    endpoints: Vec<String>,
    proxy: Option<String>,
}

#[derive(Deserialize, Debug)]
struct UpdaterPluginConfig {
    endpoints: Vec<String>,
    #[serde(default)]
    proxy: Option<String>,
}

fn get_proxy_prefix(app_handle: &AppHandle) -> Option<String> {
    let plugins = &app_handle.config().plugins.0;
    let updater_value = plugins.get("updater")?;
    let config: UpdaterPluginConfig = serde_json::from_value(updater_value.clone()).ok()?;
    config.proxy
}

#[tauri::command]
fn get_updater_config(app_handle: AppHandle) -> Result<UpdaterConfig, String> {
    let plugins = &app_handle.config().plugins.0;
    let updater_value = plugins.get("updater").ok_or("updater config not found")?;
    let config: UpdaterPluginConfig = serde_json::from_value(updater_value.clone())
        .map_err(|e| format!("failed to parse updater config: {}", e))?;

    Ok(UpdaterConfig {
        endpoints: config.endpoints,
        proxy: config.proxy,
    })
}

#[tauri::command]
async fn download_and_install_update(
    app_handle: AppHandle,
    url: String,
    _signature: String,
    version: String,
) -> Result<(), String> {
    info!("开始下载更新 v{}: {}", version, url);
    let client = reqwest::Client::builder()
        .user_agent("miniserve-gui-updater")
        .build()
        .map_err(|e| e.to_string())?;

    let proxy_prefix = get_proxy_prefix(&app_handle).unwrap_or_default();
    let download_url = if !proxy_prefix.is_empty() && url.contains("github.com") {
        format!("{}{}", proxy_prefix, url)
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

    let temp_dir = std::env::temp_dir();
    let file_name = url.split('/').last().unwrap_or("update.exe");
    let temp_path = temp_dir.join(file_name);

    fs::write(&temp_path, &bytes).map_err(|e| e.to_string())?;
    info!("更新已下载到: {:?}", temp_path);

    #[cfg(windows)]
    {
        use std::process::Command;
        let install_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));

        let mut cmd = Command::new(&temp_path);
        cmd.arg("/S");
        if let Some(dir) = install_dir {
            cmd.arg(format!("/D={}", dir.display()));
        }

        let status = cmd.spawn().map_err(|e| format!("启动安装程序失败: {}", e))?;
        info!("安装程序已启动: {:?}", status);
    }

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        use std::process::Command;

        // 设置可执行权限
        let mut perms = fs::metadata(&temp_path).map_err(|e| e.to_string())?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&temp_path, perms).map_err(|e| e.to_string())?;

        let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let target_path = current_exe.to_string_lossy().to_string();
        let source_path = temp_path.to_string_lossy().to_string();

        // 使用 sh -c 先删除旧文件（Linux允许删除正在运行的文件），再复制新文件
        let cmd = format!("rm -f '{}' && cp '{}' '{}'", target_path, source_path, target_path);
        let output = Command::new("pkexec")
            .args(["sh", "-c", &cmd])
            .output();

        match output {
            Ok(o) if o.status.success() => {
                info!("更新安装成功，重启应用");
                app_handle.restart();
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                log::error!("pkexec 失败: {}", stderr);
                return Err(format!("更新失败: {}", stderr.trim()));
            }
            Err(e) => {
                return Err(format!("pkexec 执行失败: {}", e));
            }
        }
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
fn get_package_type() -> String {
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
        use std::process::Command;
        // 通过查询注册表判断是否安装了此程序 (使用 tauri.conf.json 中的 identifier)
        let is_installed = |root: &str| {
            Command::new("reg")
                .args(["query", &format!("{}\\{}", root, "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\com.kim.miniserve-gui")])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        };

        if is_installed("HKCU") || is_installed("HKLM") {
            return "installer".to_string();
        }
        return "portable".to_string();
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    "unknown".to_string()
}

// ============ App Entry ============

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 只在 debug 模式下初始化 logger，避免 release 模式弹出控制台
    #[cfg(debug_assertions)]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    #[cfg(debug_assertions)]
    info!("miniserve-gui starting...");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 当第二个实例启动时，显示已存在的窗口
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState::default())
        .setup(|app| {
            // 创建托盘菜单
            let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            // 创建系统托盘
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("miniserve-gui")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "quit" => {
                        let state = app.state::<AppState>();
                        if let Ok(mut child_guard) = state.child.lock() {
                            if let Some(mut c) = child_guard.take() {
                                let _ = c.kill();
                                let _ = c.wait();
                            }
                        }
                        #[cfg(windows)]
                        {
                            if let Ok(mut job_guard) = state.job_handle.lock() {
                                if let Some(job) = job_guard.take() {
                                    job_object::close_job(job);
                                }
                            }
                        }
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                })
                .build(app)?;

            // 拦截窗口关闭事件，改为隐藏到托盘
            let win = app.get_webview_window("main").unwrap();
            let win_clone = win.clone();
            win.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    // 阻止默认关闭行为
                    api.prevent_close();
                    // 隐藏窗口
                    let _ = win_clone.hide();
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_engine_status,
            download_engine,
            load_config,
            save_config,
            start_server,
            stop_server,
            get_server_status,
            generate_qr,
            get_install_dir,
            get_updater_config,
            download_and_install_update,
            get_package_type,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
