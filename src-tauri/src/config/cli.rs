use std::{
    io::Read,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
pub fn run(args: &[&str]) -> Result<String, String> {
    let mut output = tempfile::tempfile().map_err(|_| "无法创建 CLI 输出缓冲")?;
    let mut child = Command::new("droid")
        .args(args)
        .stdin(Stdio::null())
        .stdout(output.try_clone().map_err(|_| "无法准备 CLI 输出")?)
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "未找到 Droid CLI，请安装后重新启动应用并检查 PATH")?;
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
