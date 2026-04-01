// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 仅在 Linux 下禁用 WebKit 合成器，解决 VMware 虚拟机渲染问题
    #[cfg(target_os = "linux")]
    std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");

    miniserve_gui_lib::run();
}
