use std::process::Child;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

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
    pub color_scheme: String,
    pub title: String,
    pub compress: String,
    pub hidden: bool,
    #[serde(default)]
    pub random_route: bool,
    #[serde(default)]
    pub readme: bool,
    #[serde(default)]
    pub download: bool,
    #[serde(default)]
    pub webdav: bool,
    #[serde(default = "default_github_proxy")]
    pub github_proxy: String,
}

fn default_github_proxy() -> String {
    "https://github.369900.xyz/".into()
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
            color_scheme: "squirrel".into(),
            title: "miniserve".into(),
            compress: "".into(),
            hidden: false,
            random_route: false,
            readme: false,
            download: false,
            webdav: false,
            github_proxy: default_github_proxy(),
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
    pub url: Option<String>,
    pub urls: Vec<String>,
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
    /// 防止并发启动：同一时间只允许一个 start_server 执行
    pub starting: AtomicBool,
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
            starting: AtomicBool::new(false),
            #[cfg(windows)]
            job_handle: Mutex::new(None),
        }
    }
}

impl AppState {
    /// Kill child process and clean up job object. Idempotent.
    pub fn kill_child(&self) -> Result<(), String> {
        let mut child_guard = self.child.lock().map_err(|e| e.to_string())?;
        if let Some(mut c) = child_guard.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        drop(child_guard);

        #[cfg(windows)]
        {
            if let Ok(mut job_guard) = self.job_handle.lock() {
                if let Some(job) = job_guard.take() {
                    crate::job_object::close_job(job);
                }
            }
        }
        Ok(())
    }
}
