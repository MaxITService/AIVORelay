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

    let exe_path = std::env::current_exe().map_err(|error| error.to_string())?;
    let exe_dir = exe_path.parent().ok_or("Cannot resolve executable directory")?;
    let is_portable = exe_dir
        .join("portable")
        .try_exists()
        .map_err(|error| format!("Cannot read portable marker: {error}"))?;
    let data_dir = if is_portable && configured_mode(exe_dir)? == StorageMode::Portable {
        let data_dir = exe_dir.join("Data");
        std::fs::create_dir_all(&data_dir)
            .map_err(|error| format!("Cannot create portable data directory: {error}"))?;
        let portable_hf_home = hugging_face_home(&data_dir);
        if let Some(home) = &hf_home {
            std::env::set_var(ORIGINAL_HF_HOME_ENV, home);
        }
        std::env::set_var(REDIRECTED_HF_HOME_ENV, &portable_hf_home);
        std::env::set_var("HF_HOME", portable_hf_home);
        Some(data_dir)
    } else {
        None
    };
    PORTABLE_EXE_DIR.get_or_init(|| is_portable.then(|| exe_dir.to_path_buf()));
    PORTABLE_DATA_DIR.get_or_init(|| data_dir);
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
    use super::{configured_mode, hugging_face_home, original_hf_home, save_storage_mode, StorageMode};
    use std::path::Path;

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
}
