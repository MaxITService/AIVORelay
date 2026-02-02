//! Tauri commands for region capture overlay communication.

#[cfg(target_os = "windows")]
use crate::region_capture::{
    on_region_cancelled, on_region_selected, ManagedRegionCaptureState, SelectedRegion,
    VirtualScreenInfo,
};

#[cfg(not(target_os = "windows"))]
use specta::Type;

use tauri::{AppHandle, Manager};

/// Selected region for non-Windows platforms (stub)
#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Type)]
pub struct SelectedRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Virtual screen info for non-Windows platforms (stub)
#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone, serde::Serialize, Type)]
pub struct VirtualScreenInfo {
    pub offset_x: i32,
    pub offset_y: i32,
    pub total_width: u32,
    pub total_height: u32,
    pub scale_factor: f64,
}

/// Response for get_data command
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct RegionCaptureData {
    pub screenshot: Option<String>, // base64 (legacy mode only)
    pub virtual_screen: VirtualScreenInfo,
}

/// Called from the overlay to get screenshot data when ready.
#[tauri::command]
#[specta::specta]
pub fn region_capture_get_data(app: AppHandle) -> Result<RegionCaptureData, String> {
    #[cfg(target_os = "windows")]
    {
        use crate::region_capture::base64_encode;

        let state = app.state::<ManagedRegionCaptureState>();
        let guard = state.lock().unwrap();

        let virtual_info = guard
            .virtual_info
            .as_ref()
            .ok_or("No virtual screen info available")?;

        Ok(RegionCaptureData {
            screenshot: guard
                .screenshot_data
                .as_ref()
                .map(|data| base64_encode(data)),
            virtual_screen: virtual_info.clone(),
        })
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("Region capture is only supported on Windows".to_string())
    }
}

/// Called from the overlay when user confirms region selection.
#[tauri::command]
#[specta::specta]
pub fn region_capture_confirm(app: AppHandle, region: SelectedRegion) {
    #[cfg(target_os = "windows")]
    on_region_selected(&app, region);

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, region);
        log::warn!("region_capture_confirm called on non-Windows platform");
    }
}

/// Called from the overlay when user cancels region capture.
#[tauri::command]
#[specta::specta]
pub fn region_capture_cancel(app: AppHandle) {
    #[cfg(target_os = "windows")]
    on_region_cancelled(&app);

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        log::warn!("region_capture_cancel called on non-Windows platform");
    }
}
