//! Private file primitives. Backups/JSON validation belong to the config apply layer.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read, Write},
    path::Path,
    time::UNIX_EPOCH,
};

pub const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fingerprint {
    pub sha256: String,
    pub size: u64,
    // A decimal string avoids loss of nanosecond precision over the JS boundary.
    pub modified_ns: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSnapshot {
    pub text: String,
    pub fingerprint: Fingerprint,
}

fn regular_file(path: &Path) -> io::Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "regular file required",
        ));
    }
    Ok(metadata)
}

pub fn read_snapshot(path: &Path) -> io::Result<FileSnapshot> {
    regular_file(path)?;
    let mut file = fs::File::open(path)?;
    let before = file.metadata()?;
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file too large"));
    }
    let after = file.metadata()?;
    if before.len() != after.len() || before.modified()? != after.modified()? {
        return Err(io::Error::other("file changed during read"));
    }
    let fingerprint = Fingerprint {
        sha256: Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        size: bytes.len() as u64,
        modified_ns: after
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos()
            .to_string(),
    };
    let text = String::from_utf8(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "UTF-8 required"))?;
    Ok(FileSnapshot { text, fingerprint })
}

/// Stage in the destination directory; an error before persist leaves the target intact.
/// This primitive does not promise crash durability of the directory rename, or create backups.
pub fn atomic_replace(path: &Path, contents: &[u8]) -> io::Result<()> {
    if contents.len() as u64 > MAX_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "file too large",
        ));
    }
    staged_write(path, |file| file.write_all(contents))
}

fn staged_write(
    path: &Path,
    write: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<()> {
    match regular_file(path) {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::Builder::new()
        .prefix(".droid-switch-")
        .tempfile_in(parent)?;
    super::permissions::restrict_file(temp.path())?;
    write(temp.as_file_mut())?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fs_replaces_existing_without_leaving_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        atomic_replace(&path, b"old").unwrap();
        atomic_replace(&path, b"new").unwrap();
        assert_eq!(read_snapshot(&path).unwrap().text, "new");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn fs_hash_detects_equal_length_edits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        atomic_replace(&path, b"old").unwrap();
        let before = read_snapshot(&path).unwrap().fingerprint;
        fs::write(&path, b"new").unwrap();
        let after = read_snapshot(&path).unwrap().fingerprint;
        assert_eq!(before.size, after.size);
        assert_ne!(before.sha256, after.sha256);
    }

    #[test]
    fn fs_failed_write_preserves_original() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        atomic_replace(&path, b"original").unwrap();
        assert!(atomic_replace(&path, &vec![0; MAX_FILE_BYTES as usize + 1]).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert!(atomic_replace(&dir.path().join("missing/config"), b"new").is_err());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn fs_partial_write_failure_removes_temp_and_preserves_target() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        atomic_replace(&path, b"original").unwrap();
        let result = staged_write(&path, |file| {
            file.write_all(b"partial")?;
            Err(io::Error::other("injected write failure"))
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn fs_rejects_invalid_utf8_and_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        fs::write(&path, [255]).unwrap();
        assert!(read_snapshot(&path).is_err());
        assert!(atomic_replace(dir.path(), b"new").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn fs_permissions_and_symlink_protection() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("private");
        atomic_replace(&path, b"secret").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let link = dir.path().join("link");
        symlink(&path, &link).unwrap();
        assert!(atomic_replace(&link, b"changed").is_err());
        assert!(read_snapshot(&link).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"secret");
    }
}
