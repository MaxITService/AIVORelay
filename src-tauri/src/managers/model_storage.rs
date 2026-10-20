//! Disk accounting and bounded paths for user-requested local model cleanup.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct ModelCleanupLease<'a>(&'a AtomicBool);

impl Drop for ModelCleanupLease<'_> {
    fn drop(&mut self) { self.0.store(false, Ordering::SeqCst); }
}

/// Caller holds the batch registry mutex, also used by batch admission.
pub(crate) fn claim_model_cleanup(flag: &AtomicBool, batch_active: bool) -> io::Result<ModelCleanupLease<'_>> {
    if batch_active {
        return Err(io::Error::other("Wait for the active TTS batch to finish before cleaning up models"));
    }
    flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| io::Error::other("Another local model cleanup is already running"))?;
    Ok(ModelCleanupLease(flag))
}

#[derive(Clone, serde::Serialize, specta::Type)]
pub struct LocalModelStorageEntry {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub bytes: u64,
    pub bytes_known: bool,
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
    storage_bytes_inner(path, &mut std::collections::HashSet::new())
}

fn storage_bytes_inner(path: &Path, seen: &mut std::collections::HashSet<(u64, u64)>) -> io::Result<u64> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    if is_link(&metadata) {
        return Ok(0);
    }
    if metadata.is_file() {
        if metadata.len() >= 1024 * 1024 {
            if let Some(identity) = file_identity(path, &metadata) {
                if !seen.insert(identity) { return Ok(0); }
            }
        }
        return Ok(metadata.len());
    }
    let mut bytes = 0_u64;
    for entry in fs::read_dir(path)? {
        bytes = bytes.saturating_add(storage_bytes_inner(&entry?.path(), seen)?);
    }
    Ok(bytes)
}

#[cfg(windows)]
pub(super) fn file_identity(path: &Path, _metadata: &fs::Metadata) -> Option<(u64, u64)> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION};
    let file = fs::File::open(path).ok()?;
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // `file` owns the handle throughout the native metadata query.
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut information).ok()?; }
    Some((u64::from(information.dwVolumeSerialNumber),
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow)))
}

#[cfg(unix)]
pub(super) fn file_identity(_path: &Path, metadata: &fs::Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    Some((metadata.dev(), metadata.ino()))
}

#[cfg(not(any(windows, unix)))]
pub(super) fn file_identity(_path: &Path, _metadata: &fs::Metadata) -> Option<(u64, u64)> { None }

pub(crate) fn local_mirror_is_shared<'a>(root: &Path, filename: &str, owners: impl IntoIterator<Item = &'a str>) -> bool {
    let target = root.join(filename);
    owners.into_iter().any(|owner| {
        let other = root.join(owner);
        if let (Ok(target), Ok(other)) = (target.canonicalize(), other.canonicalize()) {
            return target == other;
        }
        #[cfg(windows)]
        { target.to_string_lossy().replace('/', "\\").to_lowercase() == other.to_string_lossy().replace('/', "\\").to_lowercase() }
        #[cfg(not(windows))]
        { target == other }
    })
}

pub(crate) fn remove_hf_local_files<'a>(root: &Path, filename: &str, owners: impl IntoIterator<Item = &'a str>) -> io::Result<bool> {
    if local_mirror_is_shared(root, filename, owners) { return Ok(false); }
    remove_model_files(root, filename, false)
}

/// Preserve the ownership marker on partial failure so cleanup can be retried.
pub(crate) fn remove_owned_model_tree(root: &Path) -> io::Result<()> {
    remove_tree_preserving_marker(root, ".aivorelay-local-tts")
}

pub(crate) fn model_directory_is_complete(path: &Path) -> bool {
    path.is_dir() && !path.join(".aivorelay-incomplete-cleanup").exists()
}

fn remove_tree_preserving_marker(root: &Path, marker_name: &str) -> io::Result<()> {
    if let Some(parent) = root.parent() { reject_link(parent)?; }
    reject_link(root)?;
    if !root.exists() { return Ok(()); }
    let marker = root.join(marker_name);
    reject_link(&marker)?;
    let marker_contents = fs::read(&marker).map_err(|error| io::Error::new(error.kind(),
        "Refusing to delete a directory without a readable AivoRelay ownership marker"))?;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.path() == marker { continue; }
        let metadata = entry.path().symlink_metadata()?;
        if is_link(&metadata) {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x10 != 0 {
                    fs::remove_dir(entry.path())?;
                } else {
                    fs::remove_file(entry.path())?;
                }
            }
            #[cfg(not(windows))]
            fs::remove_file(entry.path())?;
        } else if metadata.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    fs::remove_file(&marker)?;
    if let Err(error) = fs::remove_dir(root) {
        fs::write(&marker, marker_contents).map_err(|restore| io::Error::new(restore.kind(),
            format!("Failed to remove model directory ({error}) and restore its ownership marker ({restore})")))?;
        return Err(error);
    }
    Ok(())
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

pub(crate) fn remove_model_files(root: &Path, filename: &str, is_directory: bool) -> io::Result<bool> {
    // Validate every target before the first removal. No caller-supplied
    // absolute paths, parent traversal, or linked model roots are accepted.
    let model = managed_child(root, filename)?;
    let partial = managed_child(root, &format!("{filename}.partial"))?;
    let extracting = managed_child(root, &format!("{filename}.extracting"))?;
    let mut removed = false;
    if model.exists() {
        if is_directory {
            // Reserve an incomplete marker before any destructive traversal.
            // It survives partial failure, including across app restarts.
            let marker = model.join(".aivorelay-incomplete-cleanup");
            reject_link(&marker)?;
            fs::write(marker, b"AivoRelay model cleanup is incomplete\n")?;
            remove_tree_preserving_marker(&model, ".aivorelay-incomplete-cleanup")?;
        } else {
            fs::remove_file(model)?;
        }
        removed = true;
    }
    if partial.exists() {
        fs::remove_file(partial)?;
        removed = true;
    }
    if is_directory && extracting.exists() {
        fs::remove_dir_all(extracting)?;
        removed = true;
    }
    Ok(removed)
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

    #[test]
    fn cleanup_removes_only_selected_model_and_its_partial_files() {
        let temp = Temp::new();
        for folder in ["selected", "selected.extracting", "keep", "keep.extracting"] {
            fs::create_dir(temp.0.join(folder)).unwrap();
            fs::write(temp.0.join(folder).join("weights"), [0; 7]).unwrap();
        }
        for file in ["selected.partial", "keep.partial", "recording.wav"] {
            fs::write(temp.0.join(file), [0; 3]).unwrap();
        }
        assert!(remove_model_files(&temp.0, "selected", true).unwrap());
        assert!(!temp.0.join("selected").exists());
        assert!(!temp.0.join("selected.partial").exists());
        assert!(!temp.0.join("selected.extracting").exists());
        assert!(temp.0.join("keep/weights").is_file());
        assert!(temp.0.join("keep.extracting/weights").is_file());
        assert!(temp.0.join("keep.partial").is_file());
        assert!(temp.0.join("recording.wav").is_file());
        assert!(!remove_model_files(&temp.0, "selected", true).unwrap());
    }

    #[test]
    fn invalid_cleanup_target_leaves_existing_files_untouched() {
        let temp = Temp::new();
        fs::write(temp.0.join("keep"), [0; 5]).unwrap();
        assert!(remove_model_files(&temp.0, "../keep", false).is_err());
        assert_eq!(storage_bytes(&temp.0).unwrap(), 5);
    }

    #[test]
    fn hf_cleanup_preserves_a_separately_owned_custom_file_and_partial() {
        let temp = Temp::new();
        fs::write(temp.0.join("custom.gguf"), [1; 11]).unwrap();
        fs::write(temp.0.join("custom.gguf.partial"), [2; 3]).unwrap();
        assert!(local_mirror_is_shared(&temp.0, "custom.gguf", ["custom.gguf"]));
        assert!(!remove_hf_local_files(&temp.0, "custom.gguf", ["custom.gguf"]).unwrap());
        assert_eq!(fs::read(temp.0.join("custom.gguf")).unwrap(), [1; 11]);
        assert_eq!(storage_bytes(&temp.0).unwrap(), 14);
        assert!(remove_hf_local_files(&temp.0, "custom.gguf", std::iter::empty()).unwrap());
        assert_eq!(storage_bytes(&temp.0).unwrap(), 0);
    }

    #[test]
    fn owned_tree_cleanup_requires_a_marker_and_removes_the_complete_tree() {
        let temp = Temp::new();
        fs::create_dir(temp.0.join("runtime")).unwrap();
        fs::write(temp.0.join("runtime/weights"), [0; 7]).unwrap();
        assert!(remove_owned_model_tree(&temp.0).is_err());
        assert!(temp.0.join("runtime/weights").is_file());
        fs::write(temp.0.join(".aivorelay-local-tts"), b"owned").unwrap();
        remove_owned_model_tree(&temp.0).unwrap();
        assert!(!temp.0.exists());
    }

    #[test]
    fn storage_accounting_counts_large_hard_links_once() {
        let temp = Temp::new();
        let file = fs::File::create(temp.0.join("weights")).unwrap();
        file.set_len(2 * 1024 * 1024).unwrap();
        fs::hard_link(temp.0.join("weights"), temp.0.join("alias")).unwrap();
        assert_eq!(storage_bytes(&temp.0).unwrap(), 2 * 1024 * 1024);
    }

    #[test]
    fn cleanup_admission_preserves_batch_ownership_and_releases_on_error() {
        let flag = AtomicBool::new(false);
        assert!(claim_model_cleanup(&flag, true).is_err());
        assert!(!flag.load(Ordering::SeqCst));
        {
            let _cleanup = claim_model_cleanup(&flag, false).unwrap();
            assert!(flag.load(Ordering::SeqCst));
            assert!(claim_model_cleanup(&flag, false).is_err());
        }
        assert!(!flag.load(Ordering::SeqCst));
        assert!(claim_model_cleanup(&flag, false).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn windows_filename_alias_cannot_remove_an_unselected_custom_model() {
        let temp = Temp::new();
        fs::write(temp.0.join("Custom.GGUF"), [0; 7]).unwrap();
        assert!(!remove_hf_local_files(&temp.0, "custom.gguf", ["Custom.GGUF"]).unwrap());
        assert!(temp.0.join("Custom.GGUF").is_file());
    }

    #[cfg(windows)]
    #[test]
    fn locked_tts_child_preserves_marker_and_cleanup_can_be_retried() {
        use std::os::windows::fs::OpenOptionsExt;
        let temp = Temp::new();
        fs::write(temp.0.join(".aivorelay-local-tts"), b"owned").unwrap();
        fs::write(temp.0.join("weights"), [0; 7]).unwrap();
        let locked = fs::OpenOptions::new().read(true).share_mode(3).open(temp.0.join("weights")).unwrap();
        assert!(remove_owned_model_tree(&temp.0).is_err());
        assert!(temp.0.join(".aivorelay-local-tts").is_file());
        assert!(temp.0.join("weights").is_file());
        drop(locked);
        remove_owned_model_tree(&temp.0).unwrap();
        assert!(!temp.0.exists());
    }

    #[cfg(windows)]
    #[test]
    fn locked_stt_partial_reports_failure_after_complete_file_removal() {
        use std::os::windows::fs::OpenOptionsExt;
        let temp = Temp::new();
        fs::write(temp.0.join("model.bin"), [0; 7]).unwrap();
        fs::write(temp.0.join("model.bin.partial"), [0; 3]).unwrap();
        let locked = fs::OpenOptions::new().read(true).share_mode(3).open(temp.0.join("model.bin.partial")).unwrap();
        assert!(remove_model_files(&temp.0, "model.bin", false).is_err());
        assert!(!temp.0.join("model.bin").exists());
        assert!(temp.0.join("model.bin.partial").is_file());
        drop(locked);
        assert!(remove_model_files(&temp.0, "model.bin", false).unwrap());
    }

    #[cfg(windows)]
    #[test]
    fn interrupted_directory_cleanup_stays_incomplete_until_retry_succeeds() {
        use std::os::windows::fs::OpenOptionsExt;
        let temp = Temp::new();
        let model = temp.0.join("model");
        fs::create_dir(&model).unwrap();
        fs::write(model.join("weights"), [0; 7]).unwrap();
        let locked = fs::OpenOptions::new().read(true).share_mode(3).open(model.join("weights")).unwrap();
        assert!(model_directory_is_complete(&model));
        assert!(remove_model_files(&temp.0, "model", true).is_err());
        assert!(!model_directory_is_complete(&model));
        fs::write(temp.0.join("another.bin"), [0; 3]).unwrap();
        remove_model_files(&temp.0, "another.bin", false).unwrap();
        // Every later scan, including after another model deletion, sees it incomplete.
        assert!(!model_directory_is_complete(&model));
        assert!(model.join(".aivorelay-incomplete-cleanup").is_file());
        drop(locked);
        remove_model_files(&temp.0, "model", true).unwrap();
        assert!(!model.exists());
    }

    #[cfg(windows)]
    #[test]
    fn tts_cleanup_unlinks_nested_junction_and_preserves_its_target() {
        use std::os::windows::process::CommandExt;
        let temp = Temp::new();
        let outside = Temp::new();
        fs::write(temp.0.join(".aivorelay-local-tts"), b"owned").unwrap();
        fs::write(outside.0.join("keep"), [0; 19]).unwrap();
        let linked = temp.0.join("model-cache");
        let result = std::process::Command::new("cmd").creation_flags(0x08000000)
            .args(["/c", "mklink", "/J"]).arg(&linked).arg(&outside.0).output().unwrap();
        assert!(result.status.success());
        remove_owned_model_tree(&temp.0).unwrap();
        assert!(!temp.0.exists());
        assert!(outside.0.join("keep").is_file());
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
        assert!(remove_model_files(&temp.0, "linked", true).is_err());
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
        assert!(remove_model_files(&temp.0, "linked", true).is_err());
        fs::remove_dir_all(&temp.0).unwrap();
        assert!(outside.0.join("keep").is_file());
    }
}
