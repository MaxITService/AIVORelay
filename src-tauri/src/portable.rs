use serde::Serialize;
use specta::Type;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tauri::Manager;

static PORTABLE_DATA_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
static PORTABLE_EXE_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
static PREVIOUS_HF_HOME: OnceLock<Option<PathBuf>> = OnceLock::new();
// A separate preference leaves the distribution's `portable` marker intact.
const DESKTOP_DATA_PREFERENCE: &str = "use-desktop-data";
const REDIRECTED_HF_HOME_ENV: &str = "AIVORELAY_PORTABLE_HF_HOME";
const ORIGINAL_HF_HOME_ENV: &str = "AIVORELAY_ORIGINAL_HF_HOME";

struct ResolvedPortablePaths {
    exe_dir: Option<PathBuf>,
    data_dir: Option<PathBuf>,
    portable_hf_home: Option<PathBuf>,
}

fn resolve_portable_paths(
    exe_dir: &Path,
    portable_allowed: bool,
) -> Result<ResolvedPortablePaths, String> {
    if !portable_allowed {
        return Ok(ResolvedPortablePaths {
            exe_dir: None,
            data_dir: None,
            portable_hf_home: None,
        });
    }

    let is_portable = exe_dir
        .join("portable")
        .try_exists()
        .map_err(|error| format!("Cannot read portable marker: {error}"))?;
    let data_dir = if is_portable && configured_mode(exe_dir)? == StorageMode::Portable {
        let data_dir = exe_dir.join("Data");
        std::fs::create_dir_all(&data_dir)
            .map_err(|error| format!("Cannot create portable data directory: {error}"))?;
        Some(data_dir)
    } else {
        None
    };
    let portable_hf_home = data_dir.as_deref().map(hugging_face_home);

    Ok(ResolvedPortablePaths {
        exe_dir: is_portable.then(|| exe_dir.to_path_buf()),
        data_dir,
        portable_hf_home,
    })
}

pub fn init() -> Result<(), String> {
    // Tauri restarts inherit our environment. Undo only our own redirection
    // before selecting this session's storage mode, preserving user overrides.
    let hf_home = original_hf_home(
        std::env::var_os("HF_HOME"),
        std::env::var_os(REDIRECTED_HF_HOME_ENV),
        std::env::var_os(ORIGINAL_HF_HOME_ENV),
    );
    match &hf_home {
        Some(home) => std::env::set_var("HF_HOME", home),
        None => std::env::remove_var("HF_HOME"),
    }
    std::env::remove_var(REDIRECTED_HF_HOME_ENV);
    std::env::remove_var(ORIGINAL_HF_HOME_ENV);
    PREVIOUS_HF_HOME.get_or_init(|| std::env::var_os("HF_HOME").map(PathBuf::from));

    if !crate::distribution::allows_portable_storage() {
        PORTABLE_EXE_DIR.get_or_init(|| None);
        PORTABLE_DATA_DIR.get_or_init(|| None);
        return Ok(());
    }

    let exe_path = std::env::current_exe().map_err(|error| error.to_string())?;
    let exe_dir = exe_path.parent().ok_or("Cannot resolve executable directory")?;
    let resolved = resolve_portable_paths(exe_dir, true)?;
    if let Some(portable_hf_home) = &resolved.portable_hf_home {
        if let Some(home) = &hf_home {
            std::env::set_var(ORIGINAL_HF_HOME_ENV, home);
        }
        std::env::set_var(REDIRECTED_HF_HOME_ENV, portable_hf_home);
        std::env::set_var("HF_HOME", portable_hf_home);
    }
    PORTABLE_EXE_DIR.get_or_init(|| resolved.exe_dir);
    PORTABLE_DATA_DIR.get_or_init(|| resolved.data_dir);
    Ok(())
}

fn original_hf_home(
    current: Option<std::ffi::OsString>,
    redirected: Option<std::ffi::OsString>,
    original: Option<std::ffi::OsString>,
) -> Option<std::ffi::OsString> {
    if redirected.is_some() && current == redirected {
        original
    } else {
        current
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum StorageMode {
    Portable,
    Desktop,
}

#[derive(Serialize, Type)]
pub struct PortableStorageStatus {
    pub active_mode: StorageMode,
    pub configured_mode: StorageMode,
    pub portable_path: String,
    pub desktop_path: String,
}

fn configured_mode(exe_dir: &Path) -> Result<StorageMode, String> {
    let use_desktop = exe_dir
        .join(DESKTOP_DATA_PREFERENCE)
        .try_exists()
        .map_err(|error| format!("Cannot read portable storage preference: {error}"))?;
    Ok(if use_desktop {
        StorageMode::Desktop
    } else {
        StorageMode::Portable
    })
}

pub fn is_portable() -> bool {
    PORTABLE_EXE_DIR.get().and_then(|dir| dir.as_ref()).is_some()
}

pub fn storage_status(app: &tauri::AppHandle) -> Result<Option<PortableStorageStatus>, String> {
    let Some(exe_dir) = PORTABLE_EXE_DIR.get().and_then(|dir| dir.as_ref()) else {
        return Ok(None);
    };
    let desktop_path = app.path().app_data_dir().map_err(|error| error.to_string())?;
    Ok(Some(PortableStorageStatus {
        active_mode: if data_dir().is_some() {
            StorageMode::Portable
        } else {
            StorageMode::Desktop
        },
        configured_mode: configured_mode(exe_dir)?,
        portable_path: exe_dir.join("Data").to_string_lossy().into_owned(),
        desktop_path: desktop_path.to_string_lossy().into_owned(),
    }))
}

pub fn set_storage_mode(mode: StorageMode) -> Result<(), String> {
    let exe_dir = PORTABLE_EXE_DIR
        .get()
        .and_then(|dir| dir.as_ref())
        .ok_or("Storage location can only be changed in the portable version")?;
    save_storage_mode(exe_dir, mode)
}

fn save_storage_mode(exe_dir: &Path, mode: StorageMode) -> Result<(), String> {
    let path = exe_dir.join(DESKTOP_DATA_PREFERENCE);
    let result = match mode {
        // Presence alone is the preference, so an interrupted write cannot
        // leave a partially serialized choice. Existing app data is untouched.
        StorageMode::Desktop => std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map(|_| ())
            .or_else(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists && path.is_file() {
                    Ok(())
                } else {
                    Err(error)
                }
            }),
        StorageMode::Portable => std::fs::remove_file(&path).or_else(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(error)
            }
        }),
    };
    result.map_err(|error| format!("Cannot save portable storage preference: {error}"))
}

/// Hugging Face home before portable redirection; legacy caches are read-only.
pub fn previous_hf_home() -> Option<&'static PathBuf> {
    PREVIOUS_HF_HOME.get().and_then(|path| path.as_ref())
}

/// Keep hf-hub downloads inside the portable data directory. hf-hub appends
/// its own `hub` component to `HF_HOME` for model snapshots and blobs.
fn hugging_face_home(data_dir: &Path) -> PathBuf {
    data_dir.join("huggingface")
}

pub fn data_dir() -> Option<&'static PathBuf> {
    PORTABLE_DATA_DIR.get().and_then(|dir| dir.as_ref())
}

pub fn app_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, tauri::Error> {
    if let Some(dir) = data_dir() {
        Ok(dir.clone())
    } else {
        app.path().app_data_dir()
    }
}

pub fn app_log_dir(app: &tauri::AppHandle) -> Result<PathBuf, tauri::Error> {
    if let Some(dir) = data_dir() {
        Ok(dir.join("logs"))
    } else {
        app.path().app_log_dir()
    }
}

pub fn app_cache_dir(app: &tauri::AppHandle) -> Result<PathBuf, tauri::Error> {
    if let Some(dir) = data_dir() {
        Ok(dir.join("cache"))
    } else {
        app.path().app_cache_dir()
    }
}

pub fn resolve_app_data(app: &tauri::AppHandle, relative: &str) -> Result<PathBuf, tauri::Error> {
    Ok(app_data_dir(app)?.join(relative))
}

pub fn store_path(relative: &str) -> PathBuf {
    if let Some(dir) = data_dir() {
        dir.join(relative)
    } else {
        PathBuf::from(relative)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        configured_mode, hugging_face_home, original_hf_home,
        resolve_portable_paths, save_storage_mode, StorageMode, DESKTOP_DATA_PREFERENCE,
    };
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn unique_temp_dir(label: &str) -> TempDirGuard {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "aivorelay-{label}-{}-{unique}-{seq}",
            std::process::id(),
        ));
        std::fs::create_dir_all(&path).unwrap();
        TempDirGuard(path)
    }

    #[test]
    fn hugging_face_home_is_inside_portable_data() {
        let data_dir = Path::new("portable-root").join("Data");

        assert_eq!(hugging_face_home(&data_dir), data_dir.join("huggingface"));
    }

    #[test]
    fn storage_choice_preserves_portable_marker_and_existing_data() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "aivorelay-storage-{}-{unique}", std::process::id(),
        ));
        std::fs::create_dir_all(root.join("Data")).unwrap();
        std::fs::write(root.join("portable"), b"").unwrap();
        let settings = root.join("Data").join("settings_store.json");
        std::fs::write(&settings, b"existing settings").unwrap();

        assert!(configured_mode(&root).unwrap() == StorageMode::Portable);
        save_storage_mode(&root, StorageMode::Desktop).unwrap();
        save_storage_mode(&root, StorageMode::Desktop).unwrap();
        assert!(configured_mode(&root).unwrap() == StorageMode::Desktop);
        assert!(root.join("portable").is_file());
        save_storage_mode(&root, StorageMode::Portable).unwrap();
        save_storage_mode(&root, StorageMode::Portable).unwrap();
        assert!(configured_mode(&root).unwrap() == StorageMode::Portable);
        assert!(root.join("portable").is_file());
        assert_eq!(std::fs::read(settings).unwrap(), b"existing settings");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn save_storage_mode_rejects_directory_preference_and_preserves_data() {
        let root = unique_temp_dir("dir-pref");
        let data_dir = root.0.join("Data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let marker = root.0.join("portable");
        std::fs::write(&marker, b"portable-marker").unwrap();
        let settings = data_dir.join("settings_store.json");
        std::fs::write(&settings, b"portable settings").unwrap();

        let pref_dir = root.0.join(DESKTOP_DATA_PREFERENCE);
        let sentinel = pref_dir.join("keep.txt");
        std::fs::create_dir_all(&pref_dir).unwrap();
        std::fs::write(&sentinel, b"do not delete").unwrap();

        let desktop_err = save_storage_mode(&root.0, StorageMode::Desktop).unwrap_err();
        assert!(desktop_err.starts_with("Cannot save portable storage preference: "));

        let portable_err = save_storage_mode(&root.0, StorageMode::Portable).unwrap_err();
        assert!(portable_err.starts_with("Cannot save portable storage preference: "));

        assert!(configured_mode(&root.0).unwrap() == StorageMode::Desktop);
        assert!(pref_dir.is_dir());
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"do not delete");
        assert!(marker.is_file());
        assert_eq!(std::fs::read(&marker).unwrap(), b"portable-marker");
        assert_eq!(std::fs::read(&settings).unwrap(), b"portable settings");
    }

    #[test]
    fn save_storage_mode_fails_when_executable_directory_is_missing() {
        let root = unique_temp_dir("missing-exe");
        let missing_exe_dir = root.0.join("missing-exe-dir");
        let sibling_desktop_data = root.0.join("desktop-data");
        std::fs::create_dir_all(&sibling_desktop_data).unwrap();
        let desktop_settings = sibling_desktop_data.join("settings_store.json");
        std::fs::write(&desktop_settings, b"desktop settings").unwrap();

        let desktop_err = save_storage_mode(&missing_exe_dir, StorageMode::Desktop).unwrap_err();
        assert!(desktop_err.starts_with("Cannot save portable storage preference: "));
        assert!(!missing_exe_dir.exists());
        assert!(!missing_exe_dir.join(DESKTOP_DATA_PREFERENCE).exists());

        assert!(configured_mode(&missing_exe_dir).unwrap() == StorageMode::Portable);
        save_storage_mode(&missing_exe_dir, StorageMode::Portable).unwrap();
        assert!(!missing_exe_dir.exists());
        assert_eq!(std::fs::read(&desktop_settings).unwrap(), b"desktop settings");
    }

    #[test]
    fn switching_storage_mode_does_not_touch_or_migrate_desktop_data() {
        let root = unique_temp_dir("no-migrate");
        let exe_dir = root.0.join("portable-app");
        let portable_data = exe_dir.join("Data");
        let portable_hf = portable_data.join("huggingface");
        let desktop_data = root.0.join("desktop-app-data");
        std::fs::create_dir_all(&portable_hf).unwrap();
        std::fs::create_dir_all(&desktop_data).unwrap();

        let marker = exe_dir.join("portable");
        std::fs::write(&marker, b"portable-marker").unwrap();
        let portable_settings = portable_data.join("settings_store.json");
        std::fs::write(&portable_settings, b"{\"mode\":\"portable\"}").unwrap();
        let portable_model = portable_hf.join("model.bin");
        std::fs::write(&portable_model, b"portable-model").unwrap();

        let desktop_settings = desktop_data.join("settings_store.json");
        std::fs::write(&desktop_settings, b"{\"mode\":\"desktop\"}").unwrap();
        let desktop_only = desktop_data.join("history.db");
        std::fs::write(&desktop_only, b"desktop-history").unwrap();

        save_storage_mode(&exe_dir, StorageMode::Desktop).unwrap();
        let pref_path = exe_dir.join(DESKTOP_DATA_PREFERENCE);
        assert!(configured_mode(&exe_dir).unwrap() == StorageMode::Desktop);
        assert!(pref_path.is_file());
        assert_eq!(std::fs::read(&pref_path).unwrap(), b"");
        assert_eq!(std::fs::read(&marker).unwrap(), b"portable-marker");
        assert_eq!(std::fs::read(&portable_settings).unwrap(), b"{\"mode\":\"portable\"}");
        assert_eq!(std::fs::read(&portable_model).unwrap(), b"portable-model");
        assert_eq!(std::fs::read(&desktop_settings).unwrap(), b"{\"mode\":\"desktop\"}");
        assert_eq!(std::fs::read(&desktop_only).unwrap(), b"desktop-history");
        assert!(!desktop_data.join("huggingface").exists());

        save_storage_mode(&exe_dir, StorageMode::Portable).unwrap();
        assert!(configured_mode(&exe_dir).unwrap() == StorageMode::Portable);
        assert!(!pref_path.exists());
        assert_eq!(std::fs::read(&marker).unwrap(), b"portable-marker");
        assert_eq!(std::fs::read(&portable_settings).unwrap(), b"{\"mode\":\"portable\"}");
        assert_eq!(std::fs::read(&portable_model).unwrap(), b"portable-model");
        assert!(!portable_data.join("history.db").exists());
        assert_eq!(std::fs::read(&desktop_settings).unwrap(), b"{\"mode\":\"desktop\"}");
        assert_eq!(std::fs::read(&desktop_only).unwrap(), b"desktop-history");
    }

    #[test]
    fn restart_restores_hf_home_without_overriding_user_changes() {
        let redirected = Some(std::ffi::OsString::from("portable/Data/huggingface"));
        let original = Some(std::ffi::OsString::from("custom-hf-home"));
        assert_eq!(
            original_hf_home(redirected.clone(), redirected.clone(), original.clone()), original,
        );
        assert_eq!(
            original_hf_home(redirected.clone(), redirected.clone(), None), None,
        );
        let changed = Some(std::ffi::OsString::from("user-changed-hf-home"));
        assert_eq!(original_hf_home(changed.clone(), redirected, original), changed);
    }

    #[test]
    fn disallowed_portable_storage_ignores_marker_and_skips_data_dir_creation() {
        let root = unique_temp_dir("packaged-guard");
        let marker = root.0.join("portable");
        std::fs::write(&marker, b"portable-marker").unwrap();

        let resolved = resolve_portable_paths(&root.0, false).unwrap();
        assert!(resolved.exe_dir.is_none());
        assert!(resolved.data_dir.is_none());
        assert!(resolved.portable_hf_home.is_none());
        assert!(!root.0.join("Data").exists());
        assert_eq!(std::fs::read(&marker).unwrap(), b"portable-marker");
    }

    #[test]
    fn allowed_portable_storage_resolves_marker_and_desktop_preference() {
        let root = unique_temp_dir("unpackaged-portable");
        let missing_marker = resolve_portable_paths(&root.0, true).unwrap();
        assert!(missing_marker.exe_dir.is_none());
        assert!(missing_marker.data_dir.is_none());
        assert!(missing_marker.portable_hf_home.is_none());

        std::fs::write(root.0.join("portable"), b"").unwrap();
        let active_portable = resolve_portable_paths(&root.0, true).unwrap();
        assert_eq!(active_portable.exe_dir.as_deref(), Some(root.0.as_path()));
        assert_eq!(
            active_portable.data_dir.as_deref(),
            Some(root.0.join("Data").as_path())
        );
        assert_eq!(
            active_portable.portable_hf_home.as_deref(),
            Some(root.0.join("Data").join("huggingface").as_path())
        );
        assert!(root.0.join("Data").is_dir());

        save_storage_mode(&root.0, StorageMode::Desktop).unwrap();
        let desktop_pref = resolve_portable_paths(&root.0, true).unwrap();
        assert_eq!(desktop_pref.exe_dir.as_deref(), Some(root.0.as_path()));
        assert!(desktop_pref.data_dir.is_none());
        assert!(desktop_pref.portable_hf_home.is_none());
    }
}
