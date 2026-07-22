use std::path::PathBuf;

use tauri::{AppHandle, Manager};

const WRY_DEFAULT_DISABLED_FEATURES: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

pub struct WebviewRuntimeConfig {
    pub data_directory: PathBuf,
    pub additional_browser_args: Option<String>,
}

pub fn config(app: &AppHandle) -> Result<WebviewRuntimeConfig, tauri::Error> {
    let remote_debugging_port = std::env::var("PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT")
        .ok()
        .map(|port| port.trim().to_string())
        .filter(|port| !port.is_empty());

    let data_directory = if let Some(portable_data_dir) = crate::portable::data_dir() {
        match remote_debugging_port.as_deref() {
            Some(port) => portable_data_dir.join(format!("webview-playwright-{port}")),
            None => portable_data_dir.join("webview"),
        }
    } else {
        let app_local_data_dir = app.path().app_local_data_dir()?;
        match remote_debugging_port.as_deref() {
            Some(port) => app_local_data_dir.join(format!("EBWebView-playwright-{port}")),
            None => app_local_data_dir.join("EBWebView"),
        }
    };

    Ok(WebviewRuntimeConfig {
        data_directory,
        // Wry only supplies its default disabled-feature arguments when custom
        // browser arguments are absent. Preserve them when enabling CDP.
        additional_browser_args: remote_debugging_port.map(|port| {
            format!("{WRY_DEFAULT_DISABLED_FEATURES} --remote-debugging-port={port}")
        }),
    })
}
