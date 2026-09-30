// Prevents additional console window on Windows in release, DO NOT REMOVE!!
// Force rebuild to apply latest user transparent icon
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 仅在 Linux 下设置 WebKit 与 GDK 环境变量，解决虚拟机及 AppImage 下画面不刷新、按钮与输入框无响应问题
    #[cfg(target_os = "linux")]
    {
        // AppImage 的 linuxdeploy-plugin-gtk.sh 默认会强制注入 GDK_BACKEND=x11，
        // 导致在 Wayland 桌面下被迫走 XWayland，引发 WebKitGTK 帧时钟死锁、界面不重绘。
        // 当检测到 Wayland 会话时，重置为 wayland,x11（优先使用原生 Wayland，不可用时回退 X11）。
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            std::env::set_var("GDK_BACKEND", "wayland,x11");
        }
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    }

    miniserve_gui_lib::run();
}
