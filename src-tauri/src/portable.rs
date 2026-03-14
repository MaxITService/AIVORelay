use std::path::PathBuf;
use std::sync::OnceLock;
use tauri::Manager;

static PORTABLE_DATA_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();

pub fn init() {
    PORTABLE_DATA_DIR.get_or_init(|| {
        let exe_path = std::env::current_exe().ok()?;
        let exe_dir = exe_path.parent()?;

        if exe_dir.join("portable").exists() {
            let data_dir = exe_dir.join("Data");
            std::fs::create_dir_all(&data_dir).ok()?;
            Some(data_dir)
        } else {
            None
        }
    });
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

pub fn resolve_app_data(
    app: &tauri::AppHandle,
    relative: &str,
) -> Result<PathBuf, tauri::Error> {
    Ok(app_data_dir(app)?.join(relative))
}

pub fn store_path(relative: &str) -> PathBuf {
    if let Some(dir) = data_dir() {
        dir.join(relative)
    } else {
        PathBuf::from(relative)
    }
}
