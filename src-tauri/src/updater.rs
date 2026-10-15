use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub download_url: Option<String>,
    pub release_notes: Option<String>,
    pub release_date: Option<String>,
}

#[tauri::command]
pub async fn check_for_updates(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    let current_version = app.package_info().version.to_string();
    
    #[cfg(not(target_os = "linux"))]
    {
        match app.updater() {
            Some(updater) => {
                match updater.check().await {
                    Ok(Some(update)) => {
                        Ok(UpdateInfo {
                            available: true,
                            current_version: current_version.clone(),
                            latest_version: Some(update.version.clone()),
                            download_url: Some(update.download_url.clone()),
                            release_notes: update.body.clone(),
                            release_date: update.date.clone(),
                        })
                    }
                    Ok(None) => {
                        Ok(UpdateInfo {
                            available: false,
                            current_version,
                            latest_version: None,
                            download_url: None,
                            release_notes: None,
                            release_date: None,
                        })
                    }
                    Err(e) => Err(format!("检查更新失败: {}", e)),
                }
            }
            None => Err("更新器未配置".to_string()),
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        // Linux 不支持自动更新，返回手动更新提示
        Ok(UpdateInfo {
            available: false,
            current_version,
            latest_version: None,
            download_url: Some("https://github.com/thin2/droidswitch/releases/latest".to_string()),
            release_notes: Some("Linux 平台请手动下载最新版本".to_string()),
            release_date: None,
        })
    }
}

#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(not(target_os = "linux"))]
    {
        match app.updater() {
            Some(updater) => {
                match updater.check().await {
                    Ok(Some(update)) => {
                        update
                            .download_and_install(|_chunk_length, _content_length| {}, || {})
                            .await
                            .map_err(|e| format!("安装更新失败: {}", e))?;
                        Ok(())
                    }
                    Ok(None) => Err("没有可用更新".to_string()),
                    Err(e) => Err(format!("检查更新失败: {}", e)),
                }
            }
            None => Err("更新器未配置".to_string()),
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        Err("Linux 平台不支持自动更新，请手动下载安装".to_string())
    }
}
