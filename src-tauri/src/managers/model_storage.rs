//! Disk accounting and bounded paths for user-requested local model cleanup.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, serde::Serialize, specta::Type)]
pub struct LocalModelStorageEntry {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub bytes: u64,
    pub shared_cache_bytes: u64,
    pub blocked_reason: Option<String>,
}

pub(crate) fn is_link(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Directory junctions are reparse points too.
        return metadata.file_attributes() & 0x400 != 0;
    }
    #[cfg(not(windows))]
    false
}

/// File lengths, without following links into shared caches or other folders.
pub(crate) fn storage_bytes(path: &Path) -> io::Result<u64> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    if is_link(&metadata) {
        return Ok(0);
    }
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    let mut bytes = 0_u64;
    for entry in fs::read_dir(path)? {
        bytes = bytes.saturating_add(storage_bytes(&entry?.path())?);
    }
    Ok(bytes)
}

/// Catalog and discovered filenames must remain under the managed model root.
pub(crate) fn managed_child(root: &Path, filename: &str) -> io::Result<PathBuf> {
    let relative = Path::new(filename);
    if relative.as_os_str().is_empty()
        || relative.components().any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid model filename"));
    }
    let mut path = root.to_path_buf();
    reject_link(&path)?;
    for part in relative.components() {
        path.push(part);
        reject_link(&path)?;
    }
    Ok(path)
}

pub(crate) fn reject_link(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_link(&metadata) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Refusing model cleanup through a symbolic link or directory junction",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "aivorelay-model-storage-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }

    #[test]
    fn measures_files_partial_downloads_and_nested_directories() {
        let temp = Temp::new();
        fs::create_dir(temp.0.join("model")).unwrap();
        fs::write(temp.0.join("model/weights"), [0; 7]).unwrap();
        fs::write(temp.0.join("weights.partial"), [0; 3]).unwrap();
        assert_eq!(storage_bytes(&temp.0).unwrap(), 10);
        assert_eq!(storage_bytes(&temp.0.join("missing")).unwrap(), 0);
    }

    #[test]
    fn managed_paths_reject_escape_and_accept_nested_weights() {
        let temp = Temp::new();
        for filename in ["", "../outside", "/outside"] {
            assert!(managed_child(&temp.0, filename).is_err());
        }
        assert_eq!(managed_child(&temp.0, "model/weights").unwrap(), temp.0.join("model/weights"));
    }

    #[cfg(windows)]
    #[test]
    fn junctions_are_not_counted_or_followed_for_cleanup() {
        use std::os::windows::process::CommandExt;
        let temp = Temp::new();
        let outside = Temp::new();
        fs::write(outside.0.join("keep"), [0; 19]).unwrap();
        let linked = temp.0.join("linked");
        let result = std::process::Command::new("cmd")
            .creation_flags(0x08000000)
            .args(["/c", "mklink", "/J"])
            .arg(&linked).arg(&outside.0).output().unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        assert_eq!(storage_bytes(&temp.0).unwrap(), 0);
        assert!(managed_child(&temp.0, "linked/keep").is_err());
        fs::remove_dir_all(&temp.0).unwrap();
        assert!(outside.0.join("keep").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn links_are_not_counted_or_followed_for_cleanup() {
        let temp = Temp::new();
        let outside = Temp::new();
        fs::write(outside.0.join("keep"), [0; 19]).unwrap();
        std::os::unix::fs::symlink(&outside.0, temp.0.join("linked")).unwrap();
        assert_eq!(storage_bytes(&temp.0).unwrap(), 0);
        assert!(managed_child(&temp.0, "linked/keep").is_err());
        fs::remove_dir_all(&temp.0).unwrap();
        assert!(outside.0.join("keep").is_file());
    }
}
