use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
// Finder/LaunchServices does not inherit interactive shell PATH customizations.
// Resolve explicit locations without running login scripts or searching the cwd.
fn candidates(path: Option<&std::ffi::OsStr>, home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = path
        .map(std::env::split_paths)
        .into_iter()
        .flatten()
        .filter(|p| p.is_absolute())
        .collect();
    if let Some(home) = home {
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join("bin"));
    }
    #[cfg(target_os = "macos")]
    dirs.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]);
    let name = if cfg!(windows) { "droid.exe" } else { "droid" };
    let mut paths = Vec::new();
    for dir in dirs {
        let file = dir.join(name);
        if !paths.contains(&file) {
            paths.push(file);
        }
    }
    paths
}
fn executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}
fn resolve() -> Result<PathBuf, String> {
    let path = std::env::var_os("PATH");
    let home =
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    let paths = candidates(path.as_deref(), home.as_deref());
    if let Some(found) = paths.iter().find(|p| executable(p)) {
        return Ok(found.clone());
    }
    if paths.iter().any(|p| p.is_file()) {
        return Err("已找到 Droid CLI 文件，但没有执行权限，请检查安装文件权限".into());
    }
    Err("未找到 Droid CLI：已检查 PATH 和用户安装目录（~/.local/bin）。若安装在自定义目录，请将其加入应用启动环境的 PATH".into())
}
pub fn run(args: &[&str]) -> Result<String, String> {
    let mut output = tempfile::tempfile().map_err(|_| "无法创建 CLI 输出缓冲")?;
    let mut child = Command::new(resolve()?)
        .args(args)
        .stdin(Stdio::null())
        .stdout(output.try_clone().map_err(|_| "无法准备 CLI 输出")?)
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::PermissionDenied => "Droid CLI 无执行权限或被系统阻止",
            std::io::ErrorKind::NotFound => "Droid CLI 文件或其依赖的启动程序不存在，请检查安装",
            _ => "Droid CLI 无法启动，请检查安装文件与系统兼容性",
        })?;
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < Duration::from_secs(15) => {
                std::thread::sleep(Duration::from_millis(25))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Droid 检测失败或超时".into());
            }
        }
    };
    if !status.success() {
        return Err("Droid 检测命令未成功".into());
    }
    use std::io::{Seek, SeekFrom};
    output
        .seek(SeekFrom::Start(0))
        .map_err(|_| "无法读取 CLI 输出")?;
    let mut text = String::new();
    output
        .take(1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|_| "CLI 输出格式无效")?;
    if text.len() > 1024 * 1024 {
        return Err("CLI 输出超过限制".into());
    }
    Ok(text)
}
pub fn version() -> Result<String, String> {
    let value = run(&["--version"])?;
    let v = value.trim();
    crate::adapter::versions::require_supported(v)?;
    Ok(v.into())
}
#[tauri::command]
pub async fn detect_droid() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(version)
        .await
        .map_err(|_| "CLI 检测任务失败")?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gui_path_finds_user_install_and_preserves_path_priority() {
        let d = tempfile::tempdir().unwrap();
        let home = d.path().join("home");
        let custom = d.path().join("custom");
        let name = if cfg!(windows) { "droid.exe" } else { "droid" };
        let installed = home.join(".local/bin").join(name);
        std::fs::create_dir_all(installed.parent().unwrap()).unwrap();
        std::fs::write(&installed, "test fixture").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&installed, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let path = std::env::join_paths([custom.as_path()]).unwrap();
        let paths = candidates(Some(&path), Some(&home));
        assert_eq!(paths[0], custom.join(name));
        assert_eq!(paths.iter().find(|p| executable(p)), Some(&installed));
        assert!(!executable(installed.parent().unwrap()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&installed, std::fs::Permissions::from_mode(0o600)).unwrap();
            assert!(!executable(&installed));
        }
    }
    #[test]
    fn ignores_relative_path_entries_and_deduplicates() {
        let d = tempfile::tempdir().unwrap();
        let bin = d.path().join(".local/bin");
        let path =
            std::env::join_paths([Path::new(""), Path::new("."), bin.as_path(), bin.as_path()])
                .unwrap();
        let paths = candidates(Some(&path), Some(d.path()));
        assert!(paths.iter().all(|p| p.is_absolute()));
        assert_eq!(
            paths
                .iter()
                .filter(|p| p.parent() == Some(bin.as_path()))
                .count(),
            1
        );
    }
}
