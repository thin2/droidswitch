use crate::{
    fs::{self, FileSnapshot, Fingerprint},
    net,
};
use serde::Deserialize;
use std::{path::Path, sync::Mutex};

static FILE_ACCESS: Mutex<()> = Mutex::new(());

#[derive(Deserialize)]
#[serde(tag = "state", content = "fingerprint", rename_all = "snake_case")]
pub enum ExpectedFile {
    Missing,
    Present(Fingerprint),
}

#[tauri::command]
pub async fn read_text_file(path: String) -> Result<FileSnapshot, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = FILE_ACCESS.lock().map_err(|_| "file_access_unavailable")?;
        fs::read_snapshot(Path::new(&path)).map_err(|_| "file_read_failed")
    })
    .await
    .map_err(|_| "file_access_unavailable")?
}

/// A low-level checked write, not the application-level Apply action (which must back up first).
/// Serializes this app's own writes. External-process TOCTOU protection is the apply layer's job.
#[tauri::command]
pub async fn write_text_file(
    path: String,
    text: String,
    expected: ExpectedFile,
) -> Result<(), &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = FILE_ACCESS.lock().map_err(|_| "file_access_unavailable")?;
        let path = Path::new(&path);
        match expected {
            ExpectedFile::Missing => match std::fs::symlink_metadata(path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                _ => return Err("file_conflict"),
            },
            ExpectedFile::Present(expected) => {
                let actual = fs::read_snapshot(path).map_err(|_| "file_conflict")?;
                if actual.fingerprint != expected {
                    return Err("file_conflict");
                }
            }
        }
        fs::atomic_replace(path, text.as_bytes()).map_err(|_| "file_write_failed")
    })
    .await
    .map_err(|_| "file_access_unavailable")?
}

#[tauri::command]
pub async fn http_request(
    request: net::HttpRequest,
) -> Result<net::HttpResponse, net::NetworkError> {
    net::execute(&net::client()?, request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn fs_command_rejects_stale_and_missing_expectations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::atomic_replace(&path, b"first").unwrap();
        let baseline = fs::read_snapshot(&path).unwrap().fingerprint;
        fs::atomic_replace(&path, b"external").unwrap();
        assert_eq!(
            write_text_file(
                path.to_string_lossy().into(),
                "mine".into(),
                ExpectedFile::Present(baseline)
            )
            .await,
            Err("file_conflict")
        );
        assert_eq!(
            write_text_file(
                path.to_string_lossy().into(),
                "mine".into(),
                ExpectedFile::Missing
            )
            .await,
            Err("file_conflict")
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"external");
    }
}
