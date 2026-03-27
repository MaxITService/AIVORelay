use serde::Serialize;
use specta::Type;
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppRuntimeInfo {
    pub executable_name: Option<String>,
    pub executable_variant: String,
    pub self_update_supported: bool,
}

pub fn get_app_runtime_info(app: &AppHandle) -> AppRuntimeInfo {
    AppRuntimeInfo {
        executable_name: current_executable_name(),
        executable_variant: current_executable_variant(),
        self_update_supported: self_update_supported(app),
    }
}

pub fn self_update_supported(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|config| config.get("endpoints"))
        .and_then(|endpoints| endpoints.as_array())
        .is_some_and(|endpoints| !endpoints.is_empty())
}

fn current_executable_name() -> Option<String> {
    std::env::current_exe()
        .ok()?
        .file_name()?
        .to_str()
        .map(str::to_owned)
}

fn current_executable_variant() -> String {
    std::env::var("AIVORELAY_VARIANT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| executable_variant_from_name(current_executable_name().as_deref()))
}

fn executable_variant_from_name(executable_name: Option<&str>) -> String {
    let normalized = executable_name.unwrap_or_default().to_ascii_lowercase();

    if normalized.contains("cuda") {
        "cuda".to_string()
    } else if normalized.contains("avx2") {
        "avx2".to_string()
    } else {
        "standard".to_string()
    }
}
