use std::process::Child;
use std::sync::Mutex;

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
    pub media_controls: bool,
    pub color_scheme: String,
    pub title: String,
    pub compress: String,
    pub hidden: bool,
    pub thumbnails: bool,
    #[serde(default)]
    pub random_route: bool,
    #[serde(default)]
    pub readme: bool,
    #[serde(default)]
    pub download: bool,
    #[serde(default)]
    pub webdav: bool,
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
            compress: "".into(),
            hidden: false,
            thumbnails: false,
            random_route: false,
            readme: false,
            download: false,
            webdav: false,
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
