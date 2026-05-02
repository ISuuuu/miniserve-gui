use std::fs::{self, File};
use std::io::Write as IoWrite;
use std::process::{Command, Stdio};
use std::thread;

use futures_util::StreamExt;
use log::info;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::state::{AppState, EngineStatus, QrCodeResponse, ServerConfig, ServerStatus};
use crate::utils::{build_miniserve_args, get_config_path, get_engine_path, get_local_ips, validate_config};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// ============ Tauri Commands ============

#[tauri::command]
pub async fn get_engine_status() -> Result<EngineStatus, String> {
    let path = get_engine_path();
    let exists = path.exists();
    let version = if exists {
        let mut cmd = Command::new(&path);
        cmd.arg("--version");
        
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
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
    // 验证配置
    validate_config(&config)?;
    
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
                crate::job_object::close_job(job);
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
    use std::time::Duration;
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
pub async fn stop_server(state: State<'_, AppState>, app_handle: AppHandle) -> Result<(), String> {
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
                crate::job_object::close_job(job);
            }
        }
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

pub fn get_proxy_prefix(app_handle: &AppHandle) -> Option<String> {
    let plugins = &app_handle.config().plugins.0;
    let updater_value = plugins.get("updater")?;
    let config: UpdaterPluginConfig = serde_json::from_value(updater_value.clone()).ok()?;
    config.proxy
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

#[tauri::command]
pub async fn download_and_install_update(
    app_handle: AppHandle,
    url: String,
    _signature: String,
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

        let source_path = temp_path.to_string_lossy().to_string();

        let output = if source_path.ends_with(".deb") {
            info!("检测到 deb 文件，使用 dpkg 安装...");
            let cmd = format!("dpkg -i '{}'", source_path);
            Command::new("pkexec")
                .args(["sh", "-c", &cmd])
                .output()
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

            // 计算新的 AppImage 文件名
            let target_path_buf = std::path::PathBuf::from(&target_path);
            let target_dir = target_path_buf.parent().unwrap().to_string_lossy().to_string();
            // 新文件名为: miniserve-gui_v{version}_x86_64.AppImage
            let new_target_name = format!("miniserve-gui_{}_x86_64.AppImage", version);
            let final_path = format!("{}/{}", target_dir, new_target_name);

            // 替换文件并重命名
            let cmd = format!("rm -f '{}' && cp '{}' '{}'", target_path, source_path, final_path);
            
            // 为了防止用户是在终端直接通过旧名字运行导致重启找不到文件
            // 我们设置一个临时的环境变量或者让 tauri 依靠重启时传入的参数，但这里最安全的是覆盖并重命名
            // 注意：如果重命名了，原有的快捷方式可能会失效。更好的做法是覆盖原内容，但不改名，或者覆盖后再建立一个同名软链接。
            // 这里我们选择：覆盖原内容，然后将文件重命名，如果用户通过双击运行，下次需点击新文件。
            Command::new("pkexec")
                .args(["sh", "-c", &cmd])
                .output()
        };

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
        // 改进 Windows 检测逻辑：检查可执行文件同级目录是否存在 Uninstall 卸载程序
        // NSIS 打包默认会生成类似 "Uninstall miniserve-gui.exe" 的文件
        let is_installer = std::env::current_exe()
            .map(|p| {
                if let Some(dir) = p.parent() {
                    dir.join("Uninstall miniserve-gui.exe").exists() 
                    || dir.join("unins000.exe").exists() 
                    || dir.to_string_lossy().to_lowercase().contains("program files")
                    || dir.to_string_lossy().to_lowercase().contains("appdata\\local\\programs")
                } else {
                    false
                }
            })
            .unwrap_or(false);

        if is_installer {
            return "installer".to_string();
        }
        return "portable".to_string();
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    "unknown".to_string()
}

#[tauri::command]
pub fn show_window_command(app_handle: AppHandle) -> Result<(), String> {
    show_window(&app_handle);
    Ok(())
}

pub fn show_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();

        #[cfg(target_os = "windows")]
        {
            let _ = win.set_always_on_top(true);
            let _ = win.set_always_on_top(false);
        }
    }
}
