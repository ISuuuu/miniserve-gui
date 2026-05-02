use std::env;
use std::path::PathBuf;

use crate::state::ServerConfig;

// ============ Helpers ============

pub fn get_engine_path() -> PathBuf {
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    let bin_dir = base.join("miniserve-gui").join("bin");
    if env::consts::OS == "windows" {
        bin_dir.join("miniserve.exe")
    } else {
        bin_dir.join("miniserve")
    }
}

pub fn get_config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("miniserve-gui").join("config.json")
}

pub fn validate_config(cfg: &ServerConfig) -> Result<(), String> {
    // 验证端口范围
    if cfg.port == 0 {
        return Err("端口号不能为 0".into());
    }
    
    // 验证路径
    if cfg.path.is_empty() {
        return Err("请选择要分享的文件夹路径".into());
    }
    
    let path = PathBuf::from(&cfg.path);
    if !path.exists() {
        return Err(format!("路径不存在: {}", cfg.path));
    }
    if !path.is_dir() {
        return Err(format!("路径不是文件夹: {}", cfg.path));
    }
    
    // 验证认证配置
    if let (Some(u), Some(p)) = (&cfg.auth_username, &cfg.auth_password) {
        if (u.is_empty() && !p.is_empty()) || (!u.is_empty() && p.is_empty()) {
            return Err("用户名和密码必须同时填写或同时留空".into());
        }
    }
    
    // 验证配色方案
    let valid_schemes = ["squirrel", "archlinux", "zenburn", "monokai"];
    if !cfg.color_scheme.is_empty() && !valid_schemes.contains(&cfg.color_scheme.as_str()) {
        return Err(format!("无效的配色方案: {}", cfg.color_scheme));
    }
    
    Ok(())
}

pub fn get_local_ips() -> Vec<String> {
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

pub fn build_miniserve_args(cfg: &ServerConfig) -> Result<Vec<String>, String> {
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
    if cfg.webdav {
        args.push("--enable-webdav".into());
    }

    // Verbose mode to show all details in logs
    args.push("-v".into());

    Ok(args)
}
