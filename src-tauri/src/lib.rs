use log::info;
use tauri::{
    AppHandle, Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

mod commands;
mod state;
mod utils;

// ============ Windows Job Object (防止子进程成为孤儿) ============

#[cfg(windows)]
pub(crate) mod job_object {
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

// ============ App Entry ============

fn show_window(app: &AppHandle) {
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
            show_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(state::AppState::default())
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
                        show_window(app);
                    }
                    "quit" => {
                        let state = app.state::<state::AppState>();
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
                        show_window(app);
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
            commands::get_engine_status,
            commands::download_engine,
            commands::load_config,
            commands::save_config,
            commands::start_server,
            commands::stop_server,
            commands::get_server_status,
            commands::generate_qr,
            commands::get_install_dir,
            commands::get_updater_config,
            commands::download_and_install_update,
            commands::get_package_type,
            commands::show_window_command,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
