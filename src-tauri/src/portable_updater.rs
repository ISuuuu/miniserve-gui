//! 便携版自动更新（zip 覆盖方案）。
//!
//! 原理（参考 NyaTerm）：
//! - 便携 zip 与安装包共用同一份 latest.json，便携版用 `windows-x86_64-portable`
//!   作为 target key，下载由前端传入的 url + signature 完成（复用 commands.rs
//!   的真代理、失败回退直连与 minisign 验签）；
//! - zip 解压到 `%TEMP%/miniserve-portable-update-<uuid>/payload`，
//!   只提取 `miniserve-gui.exe` + `portable.flag`，跳过 `data/` 目录条目；
//! - helper 是「当前 exe 的拷贝」，以
//!   `--miniserve-portable-update-helper <parentPid> <payload> <target> <workDir>` 参数
//!   spawn，主进程 `app.exit(0)`；helper 等父进程退出后原子替换 exe 并重启；
//! - helper 入口必须在 main() 最开头判定。

use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use uuid::Uuid;

use crate::utils::current_exe_dir;

/// 便携 zip 内的根目录名（构建流水线固定此名）
pub const PORTABLE_ROOT: &str = "miniserve-gui-portable";
const PORTABLE_EXE: &str = "miniserve-gui.exe";
const PORTABLE_MARKER: &str = "portable.flag";
const HELPER_FLAG: &str = "--miniserve-portable-update-helper";
const CLEANUP_ENV: &str = "MINISERVE_PORTABLE_UPDATE_CLEANUP";
const WORK_DIR_PREFIX: &str = "miniserve-portable-update-";
const MAX_ARCHIVE_ENTRIES: usize = 128;
const MAX_PAYLOAD_BYTES: u64 = 512 * 1024 * 1024;
const STALE_WORK_DIR_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug)]
pub struct StagedPortableUpdate {
    pub work_dir: PathBuf,
    pub payload_exe: PathBuf,
    pub payload_marker: PathBuf,
}

fn current_exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

/// 确认 exe 所在目录可写（便携版可能放在只读介质或受保护目录）
pub fn ensure_exe_dir_writable() -> Result<(), String> {
    let dir = current_exe_dir().ok_or_else(|| "无法定位可执行文件目录".to_string())?;
    let probe = dir.join(format!(".miniserve-update-write-test-{}", Uuid::new_v4()));
    fs::write(&probe, b"update-write-test").map_err(|e| e.to_string())?;
    fs::remove_file(&probe).map_err(|e| e.to_string())?;
    Ok(())
}

/// 解压并暂存已验签的便携 zip。暂存的 payload exe 自身充当更新 helper，
/// 因此不需要额外拷贝一份旧版 exe。返回暂存信息，主进程随后 spawn helper 并退出。
pub fn stage_archive(zip_path: &Path) -> Result<StagedPortableUpdate, String> {
    let work_dir = std::env::temp_dir().join(format!("{WORK_DIR_PREFIX}{}", Uuid::new_v4()));
    fs::create_dir(&work_dir).map_err(|e| e.to_string())?;

    let result = (|| {
        let payload_dir = work_dir.join("payload");
        fs::create_dir(&payload_dir).map_err(|e| e.to_string())?;
        extract_portable_payload(zip_path, &payload_dir)?;

        Ok(StagedPortableUpdate {
            payload_exe: payload_dir.join(PORTABLE_EXE),
            payload_marker: payload_dir.join(PORTABLE_MARKER),
            work_dir: work_dir.clone(),
        })
    })();

    if result.is_err() {
        let _ = fs::remove_dir_all(&work_dir);
    }
    result
}

/// spawn helper 完成 exe 替换与重启，主进程随后必须退出。
pub fn apply_staged(staged: &StagedPortableUpdate, app: &tauri::AppHandle) -> Result<(), String> {
    if !staged.payload_exe.is_file() || !staged.payload_marker.is_file() {
        let _ = fs::remove_dir_all(&staged.work_dir);
        return Err("便携版更新文件不完整".into());
    }

    let target_exe = current_exe()?;
    // helper 就是 payload 里那份新版本 exe：它必定能解析当前的 helper 参数协议
    let spawn_result = Command::new(&staged.payload_exe)
        .arg(HELPER_FLAG)
        .arg(std::process::id().to_string())
        .arg(&target_exe)
        .arg(&staged.work_dir)
        .spawn();

    if let Err(error) = spawn_result {
        let _ = fs::remove_dir_all(&staged.work_dir);
        return Err(error.to_string());
    }

    app.exit(0);
    Ok(())
}

/// 安全解压便携 zip：只提取 exe + portable.flag，跳过 data/，拒绝路径逃逸与符号链接。
fn extract_portable_payload(zip_path: &Path, destination: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("便携版更新包无法读取: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("便携版更新包无效: {e}"))?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err("便携版更新包内文件数量超出上限".into());
    }

    let mut found_exe = false;
    let mut found_marker = false;
    let mut payload_bytes = 0_u64;

    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|e| format!("读取便携版更新包失败: {e}"))?;
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| "便携版更新包含不安全路径".to_string())?;
        let mut components = enclosed.components();
        if components.next() != Some(Component::Normal(PORTABLE_ROOT.as_ref())) {
            return Err("便携版更新包根目录不符合预期".into());
        }
        let mut relative = PathBuf::new();
        for component in components {
            let Component::Normal(name) = component else {
                return Err("便携版更新包含不安全相对路径".into());
            };
            relative.push(name);
        }
        if relative.as_os_str().is_empty() || entry.is_dir() {
            continue;
        }
        if entry.is_symlink() {
            return Err("便携版更新包含符号链接".into());
        }
        if relative.starts_with("data") {
            continue;
        }

        let output = if relative == Path::new(PORTABLE_EXE) {
            if found_exe {
                return Err("便携版更新包包含重复的 miniserve-gui.exe".into());
            }
            found_exe = true;
            destination.join(PORTABLE_EXE)
        } else if relative == Path::new(PORTABLE_MARKER) {
            if found_marker {
                return Err("便携版更新包包含重复的 portable.flag".into());
            }
            found_marker = true;
            destination.join(PORTABLE_MARKER)
        } else {
            return Err(format!("便携版更新包包含意外文件: {}", relative.display()));
        };

        payload_bytes = payload_bytes.saturating_add(entry.size());
        if payload_bytes > MAX_PAYLOAD_BYTES {
            return Err("便携版更新包大小超出允许上限".into());
        }

        let mut file = fs::File::create(output).map_err(|e| e.to_string())?;
        let copied = std::io::copy(&mut entry.take(MAX_PAYLOAD_BYTES + 1), &mut file)
            .map_err(|e| e.to_string())?;
        if copied > MAX_PAYLOAD_BYTES {
            return Err("便携版更新包单文件超出允许上限".into());
        }
    }

    if !found_exe || !found_marker {
        return Err("便携版更新包缺少 miniserve-gui.exe 或 portable.flag".into());
    }
    Ok(())
}

/// main() 最开头调用。若当前进程是更新 helper 则执行替换并返回 true。
pub fn run_helper_if_requested() -> bool {
    let args: Vec<OsString> = std::env::args_os().collect();
    if args.get(1).and_then(|arg| arg.to_str()) != Some(HELPER_FLAG) {
        return false;
    }

    if let Err(error) = run_helper(&args) {
        let target = args.get(3).map(PathBuf::from);
        if let Some(target_exe) = target.as_deref() {
            write_helper_error(target_exe, &error);
            // 即使替换失败也要拉起目标程序，保证用户还能启动应用
            let mut command = Command::new(target_exe);
            if let Some(target_dir) = target_exe.parent() {
                command.current_dir(target_dir);
            }
            if let Some(work_dir) = args.get(4) {
                command.env(CLEANUP_ENV, work_dir);
            }
            let _ = command.spawn();
        }
    }
    true
}

/// helper 自身的路径即新版本 exe，是替换源文件。正在运行的可执行文件在 Windows 上
/// 无法被删除，因此先复制成临时名再走「重命名 + 回滚」提交。
fn run_helper(args: &[OsString]) -> Result<(), String> {
    if args.len() != 5 {
        return Err("便携版更新 helper 参数无效".into());
    }
    let parent_pid = args[2]
        .to_str()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| "便携版更新父进程 ID 无效".to_string())?;
    let target_exe = PathBuf::from(&args[3]);
    let work_dir = PathBuf::from(&args[4]);

    wait_for_process_exit(parent_pid)?;
    replace_executable(&current_exe()?, &target_exe)?;
    let mut command = Command::new(&target_exe);
    if let Some(target_dir) = target_exe.parent() {
        command.current_dir(target_dir);
    }
    command
        .env(CLEANUP_ENV, &work_dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn replace_executable(source_exe: &Path, target_exe: &Path) -> Result<(), String> {
    let target_dir = target_exe
        .parent()
        .ok_or_else(|| "便携版可执行文件无上级目录".to_string())?;
    let new_exe = target_dir.join(".miniserve-update-new.exe");
    let backup_exe = target_dir.join(".miniserve-update-backup.exe");
    let _ = fs::remove_file(&new_exe);
    let _ = fs::remove_file(&backup_exe);
    fs::copy(source_exe, &new_exe).map_err(|e| e.to_string())?;

    commit_executable(&new_exe, target_exe, &backup_exe, |from, to| {
        fs::rename(from, to)
    })?;
    let _ = fs::remove_file(backup_exe);
    // portable.flag 是便携模式的唯一判定依据，一旦缺失 exe 就会静默退化为安装版，
    // 因此用同样「临时名 + 重命名」的方式写回当前 helper 所在目录里的那份
    let source_dir = source_exe.parent().ok_or_else(|| "便携版可执行文件无上级目录".to_string())?;
    let source_marker = source_dir.join(PORTABLE_MARKER);
    let target_marker = target_dir.join(PORTABLE_MARKER);
    let new_marker = target_dir.join(".miniserve-update-new.flag");
    let _ = fs::remove_file(&new_marker);
    fs::copy(&source_marker, &new_marker).map_err(|e| e.to_string())?;
    fs::rename(&new_marker, &target_marker).map_err(|e| e.to_string())?;
    Ok(())
}

fn commit_executable<F>(
    new_exe: &Path,
    target_exe: &Path,
    backup_exe: &Path,
    move_new: F,
) -> Result<(), String>
where
    F: FnOnce(&Path, &Path) -> std::io::Result<()>,
{
    fs::rename(target_exe, backup_exe).map_err(|e| e.to_string())?;
    if let Err(error) = move_new(new_exe, target_exe) {
        if let Err(rollback_error) = fs::rename(backup_exe, target_exe) {
            return Err(format!(
                "便携版更新安装失败 ({error})，且恢复原 exe 失败 ({rollback_error})"
            ));
        }
        return Err(error.to_string());
    }
    Ok(())
}

#[cfg(windows)]
fn wait_for_process_exit(process_id: u32) -> Result<(), String> {
    use windows::Win32::Foundation::{CloseHandle, E_INVALIDARG, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
    };

    let process = match unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, process_id) } {
        Ok(process) => process,
        Err(error) if error.code() == E_INVALIDARG => return Ok(()),
        Err(error) => {
            return Err(format!("打开父进程失败: {error}"));
        }
    };
    let result = unsafe { WaitForSingleObject(process, 120_000) };
    let _ = unsafe { CloseHandle(process) };
    if result != WAIT_OBJECT_0 {
        return Err("等待主进程退出超时".into());
    }
    Ok(())
}

#[cfg(not(windows))]
fn wait_for_process_exit(_process_id: u32) -> Result<(), String> {
    Err("便携版更新 helper 仅支持 Windows".into())
}

fn write_helper_error(target_exe: &Path, message: &str) {
    let Some(target_dir) = target_exe.parent() else {
        return;
    };
    let log_dir = target_dir.join("data").join("logs");
    if fs::create_dir_all(&log_dir).is_ok() {
        let _ = fs::write(log_dir.join("portable-update-error.log"), message);
    }
}

/// run() 开头调用（不阻塞）。清理上次更新残留的 work_dir 与过期目录。
pub fn schedule_cleanup_from_environment() {
    let explicit_cleanup = std::env::var_os(CLEANUP_ENV).and_then(|raw_path| {
        unsafe {
            std::env::remove_var(CLEANUP_ENV);
        }
        let path = PathBuf::from(raw_path);
        is_portable_work_dir(&path).then_some(path)
    });

    std::thread::spawn(move || {
        if let Some(path) = explicit_cleanup {
            std::thread::sleep(Duration::from_secs(3));
            let _ = fs::remove_dir_all(path);
        }
        cleanup_stale_work_dirs();
    });
}

fn is_portable_work_dir(path: &Path) -> bool {
    path.parent() == Some(std::env::temp_dir().as_path())
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(WORK_DIR_PREFIX))
}

fn cleanup_stale_work_dirs() {
    let Ok(entries) = fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_stale_directory = entry.file_type().is_ok_and(|kind| kind.is_dir())
            && is_portable_work_dir(&path)
            && entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|age| age >= STALE_WORK_DIR_AGE);
        if is_stale_directory {
            let _ = fs::remove_dir_all(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;

    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("msgui-{name}-{}", Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        path
    }

    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        for (name, bytes) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn write_archive(dir: &Path, entries: &[(&str, &[u8])]) -> PathBuf {
        let path = dir.join("update.zip");
        fs::write(&path, archive(entries)).unwrap();
        path
    }

    #[test]
    fn extracts_program_files_without_touching_data() {
        let destination = test_dir("portable-extract");
        let zip_path = write_archive(
            &destination,
            &[
                (concat!("miniserve-gui-portable/", "miniserve-gui.exe"), b"new-exe"),
                (concat!("miniserve-gui-portable/", "portable.flag"), b""),
                (concat!("miniserve-gui-portable/", "data/.keep"), b"package-data"),
            ],
        );

        extract_portable_payload(&zip_path, &destination).unwrap();

        assert_eq!(
            fs::read(destination.join(PORTABLE_EXE)).unwrap(),
            b"new-exe"
        );
        assert!(destination.join(PORTABLE_MARKER).is_file());
        assert!(!destination.join("data").exists());
        fs::remove_dir_all(destination).unwrap();
    }

    #[test]
    fn rejects_unsafe_or_incomplete_archives() {
        let destination = test_dir("portable-invalid");
        let unsafe_archive = write_archive(
            &destination,
            &[
                ("../miniserve-gui.exe", b"bad"),
                (concat!("miniserve-gui-portable/", "portable.flag"), b""),
            ],
        );
        assert!(extract_portable_payload(&unsafe_archive, &destination).is_err());

        let nested_escape = write_archive(
            &destination,
            &[
                (concat!("miniserve-gui-portable/", "miniserve-gui.exe"), b"new-exe"),
                (concat!("miniserve-gui-portable/", "portable.flag"), b""),
                (concat!("miniserve-gui-portable/", "data/../../escape"), b"bad"),
            ],
        );
        assert!(extract_portable_payload(&nested_escape, &destination).is_err());

        let missing_marker =
            write_archive(&destination, &[(concat!("miniserve-gui-portable/", "miniserve-gui.exe"), b"new-exe")]);
        assert!(extract_portable_payload(&missing_marker, &destination).is_err());
        fs::remove_dir_all(destination).unwrap();
    }

    #[test]
    fn executable_commit_rolls_back_when_final_move_fails() {
        let directory = test_dir("portable-rollback");
        let target = directory.join(PORTABLE_EXE);
        let new_exe = directory.join("new.exe");
        let backup = directory.join("backup.exe");
        fs::write(&target, b"old").unwrap();
        fs::write(&new_exe, b"new").unwrap();

        let result = commit_executable(&new_exe, &target, &backup, |_, _| {
            Err(std::io::Error::other("simulated failure"))
        });

        assert!(result.is_err());
        assert_eq!(fs::read(&target).unwrap(), b"old");
        fs::remove_dir_all(directory).unwrap();
    }
}
