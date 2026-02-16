use crate::input;
use crate::managers::preview_output_mode::PreviewOutputModeStatePayload;
use crate::plus_overlay_state;
use crate::settings;
use crate::settings::{
    OverlayPosition, SonioxLivePreviewPosition, SonioxLivePreviewSize, SonioxLivePreviewTheme,
};
use specta::Type;
use serde::Serialize;
use std::sync::{LazyLock, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};

/// Counter used to cancel pending profile switch overlay auto-hide timers.
/// Each time a recording overlay is shown, this is incremented, and existing
/// profile switch timers check if their generation still matches.
static PROFILE_OVERLAY_GENERATION: AtomicU64 = AtomicU64::new(0);

#[cfg(not(target_os = "macos"))]
use log::debug;
#[cfg(target_os = "windows")]
use log::info;

#[cfg(not(target_os = "macos"))]
use tauri::WebviewWindowBuilder;

#[cfg(target_os = "macos")]
use tauri::WebviewUrl;

#[cfg(target_os = "macos")]
use tauri_nspanel::{tauri_panel, CollectionBehavior, PanelBuilder, PanelLevel};

#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(RecordingOverlayPanel {
        config: {
            can_become_key_window: false,
            is_floating_panel: true
        }
    })
}

const OVERLAY_WIDTH: f64 = 172.0;
const OVERLAY_HEIGHT: f64 = 36.0;

// Command Confirmation Overlay dimensions
const COMMAND_CONFIRM_WIDTH: f64 = 520.0;
const COMMAND_CONFIRM_HEIGHT: f64 = 280.0;
const VOICE_BUTTON_WIDTH: f64 = 80.0;
const VOICE_BUTTON_HEIGHT: f64 = 80.0;
const SONIOX_LIVE_PREVIEW_SMALL_WIDTH: f64 = 560.0;
const SONIOX_LIVE_PREVIEW_SMALL_HEIGHT: f64 = 140.0;
const SONIOX_LIVE_PREVIEW_MEDIUM_WIDTH: f64 = 760.0;
const SONIOX_LIVE_PREVIEW_MEDIUM_HEIGHT: f64 = 200.0;
const SONIOX_LIVE_PREVIEW_LARGE_WIDTH: f64 = 960.0;
const SONIOX_LIVE_PREVIEW_LARGE_HEIGHT: f64 = 260.0;
const SONIOX_LIVE_PREVIEW_WINDOW_LABEL: &str = "soniox_live_preview";
const SONIOX_LIVE_PREVIEW_TOP_OFFSET: f64 = 52.0;
const SONIOX_LIVE_PREVIEW_BOTTOM_OFFSET: f64 = 86.0;
const SONIOX_LIVE_PREVIEW_CURSOR_EDGE_MARGIN: f64 = 12.0;
const SONIOX_LIVE_PREVIEW_MIN_CUSTOM_WIDTH_PX: u16 = 320;
const SONIOX_LIVE_PREVIEW_MAX_CUSTOM_WIDTH_PX: u16 = 2200;
const SONIOX_LIVE_PREVIEW_MIN_CUSTOM_HEIGHT_PX: u16 = 100;
const SONIOX_LIVE_PREVIEW_MAX_CUSTOM_HEIGHT_PX: u16 = 1400;

#[derive(Serialize, Clone)]
struct OverlayStatePayload {
    state: String,
    decapitalize_eligible: bool,
    decapitalize_armed: bool,
}

#[derive(Serialize, Clone, Default, Type)]
pub struct SonioxLivePreviewPayload {
    pub final_text: String,
    pub interim_text: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SonioxLivePreviewMode {
    UiDemo,
    Live,
}

#[derive(Clone, Copy)]
struct SonioxLivePreviewRuntimeState {
    mode: SonioxLivePreviewMode,
    live_active: bool,
}

impl Default for SonioxLivePreviewRuntimeState {
    fn default() -> Self {
        Self {
            mode: SonioxLivePreviewMode::UiDemo,
            live_active: false,
        }
    }
}

#[derive(Serialize, Clone, Type)]
pub struct SonioxLivePreviewAppearancePayload {
    pub theme: String,
    pub opacity_percent: u8,
    pub font_color: String,
    pub interim_font_color: String,
    pub accent_color: String,
    pub interim_opacity_percent: u8,
}

static SONIOX_LIVE_PREVIEW_STATE: LazyLock<Mutex<SonioxLivePreviewPayload>> =
    LazyLock::new(|| Mutex::new(SonioxLivePreviewPayload::default()));
static SONIOX_LIVE_PREVIEW_RUNTIME_STATE: LazyLock<Mutex<SonioxLivePreviewRuntimeState>> =
    LazyLock::new(|| Mutex::new(SonioxLivePreviewRuntimeState::default()));

fn decapitalize_indicator_eligible(settings: &settings::AppSettings) -> bool {
    settings.text_replacement_decapitalize_after_edit_key_enabled
        && settings.transcription_provider == settings::TranscriptionProvider::RemoteSoniox
        && settings.soniox_live_enabled
}

fn build_overlay_state_payload(state: &str, settings: &settings::AppSettings) -> OverlayStatePayload {
    let eligible = decapitalize_indicator_eligible(settings);
    let armed = eligible && crate::text_replacement_decapitalize::is_realtime_trigger_armed_now();
    OverlayStatePayload {
        state: state.to_string(),
        decapitalize_eligible: eligible,
        decapitalize_armed: armed,
    }
}

#[cfg(target_os = "macos")]
const OVERLAY_TOP_OFFSET: f64 = 46.0;
#[cfg(any(target_os = "windows", target_os = "linux"))]
const OVERLAY_TOP_OFFSET: f64 = 4.0;

#[cfg(target_os = "macos")]
const OVERLAY_BOTTOM_OFFSET: f64 = 15.0;

#[cfg(any(target_os = "windows", target_os = "linux"))]
const OVERLAY_BOTTOM_OFFSET: f64 = 40.0;

/// Forces a window to be topmost using Win32 API (Windows only)
/// This is more reliable than Tauri's set_always_on_top which can be overridden
#[cfg(target_os = "windows")]
pub fn force_overlay_topmost(overlay_window: &tauri::webview::WebviewWindow) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    };

    // Clone because run_on_main_thread takes 'static
    let overlay_clone = overlay_window.clone();

    // Make sure the Win32 call happens on the UI thread
    let _ = overlay_clone.clone().run_on_main_thread(move || {
        if let Ok(hwnd) = overlay_clone.hwnd() {
            unsafe {
                // Force Z-order: make this window topmost without changing size/pos or stealing focus
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        }
    });
}

fn get_monitor_with_cursor(app_handle: &AppHandle) -> Option<tauri::Monitor> {
    if let Some(mouse_location) = input::get_cursor_position(app_handle) {
        if let Ok(monitors) = app_handle.available_monitors() {
            for monitor in monitors {
                let is_within =
                    is_mouse_within_monitor(mouse_location, monitor.position(), monitor.size());
                if is_within {
                    return Some(monitor);
                }
            }
        }
    }

    app_handle.primary_monitor().ok().flatten()
}

fn is_mouse_within_monitor(
    mouse_pos: (i32, i32),
    monitor_pos: &PhysicalPosition<i32>,
    monitor_size: &PhysicalSize<u32>,
) -> bool {
    let (mouse_x, mouse_y) = mouse_pos;
    let PhysicalPosition {
        x: monitor_x,
        y: monitor_y,
    } = *monitor_pos;
    let PhysicalSize {
        width: monitor_width,
        height: monitor_height,
    } = *monitor_size;

    mouse_x >= monitor_x
        && mouse_x < (monitor_x + monitor_width as i32)
        && mouse_y >= monitor_y
        && mouse_y < (monitor_y + monitor_height as i32)
}

fn calculate_overlay_position(app_handle: &AppHandle) -> Option<(f64, f64)> {
    if let Some(monitor) = get_monitor_with_cursor(app_handle) {
        let work_area = monitor.work_area();
        let scale = monitor.scale_factor();
        let work_area_width = work_area.size.width as f64 / scale;
        let work_area_height = work_area.size.height as f64 / scale;
        let work_area_x = work_area.position.x as f64 / scale;
        let work_area_y = work_area.position.y as f64 / scale;

        let settings = settings::get_settings(app_handle);

        let x = work_area_x + (work_area_width - OVERLAY_WIDTH) / 2.0;
        let y = match settings.overlay_position {
            OverlayPosition::Top => work_area_y + OVERLAY_TOP_OFFSET,
            OverlayPosition::Bottom | OverlayPosition::None => {
                work_area_y + work_area_height - OVERLAY_HEIGHT - OVERLAY_BOTTOM_OFFSET
            }
        };

        return Some((x, y));
    }
    None
}

fn soniox_live_preview_dimensions(app_settings: &settings::AppSettings) -> (f64, f64) {
    match app_settings.soniox_live_preview_size {
        SonioxLivePreviewSize::Small => {
            (SONIOX_LIVE_PREVIEW_SMALL_WIDTH, SONIOX_LIVE_PREVIEW_SMALL_HEIGHT)
        }
        SonioxLivePreviewSize::Medium => {
            (SONIOX_LIVE_PREVIEW_MEDIUM_WIDTH, SONIOX_LIVE_PREVIEW_MEDIUM_HEIGHT)
        }
        SonioxLivePreviewSize::Large => {
            (SONIOX_LIVE_PREVIEW_LARGE_WIDTH, SONIOX_LIVE_PREVIEW_LARGE_HEIGHT)
        }
        SonioxLivePreviewSize::Custom => (
            app_settings
                .soniox_live_preview_custom_width_px
                .clamp(
                    SONIOX_LIVE_PREVIEW_MIN_CUSTOM_WIDTH_PX,
                    SONIOX_LIVE_PREVIEW_MAX_CUSTOM_WIDTH_PX,
                ) as f64,
            app_settings
                .soniox_live_preview_custom_height_px
                .clamp(
                    SONIOX_LIVE_PREVIEW_MIN_CUSTOM_HEIGHT_PX,
                    SONIOX_LIVE_PREVIEW_MAX_CUSTOM_HEIGHT_PX,
                ) as f64,
        ),
    }
}

fn clamp_f64(value: f64, min: f64, max: f64) -> f64 {
    if max <= min {
        return min;
    }
    value.max(min).min(max)
}

fn normalize_preview_color(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() == 7
        && trimmed.starts_with('#')
        && trimmed.chars().skip(1).all(|c| c.is_ascii_hexdigit())
    {
        return format!("#{}", trimmed[1..].to_ascii_lowercase());
    }
    fallback.to_string()
}

fn soniox_live_preview_theme_key(theme: SonioxLivePreviewTheme) -> &'static str {
    match theme {
        SonioxLivePreviewTheme::MainDark => "main_dark",
        SonioxLivePreviewTheme::Ocean => "ocean",
        SonioxLivePreviewTheme::Light => "light",
    }
}

fn build_soniox_live_preview_appearance_payload(
    app_handle: &AppHandle,
) -> SonioxLivePreviewAppearancePayload {
    let app_settings = settings::get_settings(app_handle);
    SonioxLivePreviewAppearancePayload {
        theme: soniox_live_preview_theme_key(app_settings.soniox_live_preview_theme).to_string(),
        opacity_percent: app_settings.soniox_live_preview_opacity_percent.clamp(35, 100),
        font_color: normalize_preview_color(&app_settings.soniox_live_preview_font_color, "#f5f5f5"),
        interim_font_color: normalize_preview_color(
            &app_settings.soniox_live_preview_interim_font_color,
            "#f5f5f5",
        ),
        accent_color: normalize_preview_color(
            &app_settings.soniox_live_preview_accent_color,
            "#ff4d8d",
        ),
        interim_opacity_percent: app_settings
            .soniox_live_preview_interim_opacity_percent
            .clamp(20, 95),
    }
}

#[cfg(target_os = "windows")]
fn resolve_soniox_live_preview_geometry(app_handle: &AppHandle) -> Option<(f64, f64, f64, f64)> {
    let app_settings = settings::get_settings(app_handle);
    let (width, height) = soniox_live_preview_dimensions(&app_settings);

    if app_settings.soniox_live_preview_position == SonioxLivePreviewPosition::CustomXY {
        return Some((
            app_settings.soniox_live_preview_custom_x_px as f64,
            app_settings.soniox_live_preview_custom_y_px as f64,
            width,
            height,
        ));
    }

    if let Some(monitor) = get_monitor_with_cursor(app_handle) {
        let work_area = monitor.work_area();
        let scale = monitor.scale_factor();
        let work_area_width = work_area.size.width as f64 / scale;
        let work_area_height = work_area.size.height as f64 / scale;
        let work_area_x = work_area.position.x as f64 / scale;
        let work_area_y = work_area.position.y as f64 / scale;

        let x;
        let y;

        match app_settings.soniox_live_preview_position {
            SonioxLivePreviewPosition::Top => {
                x = work_area_x + (work_area_width - width) / 2.0;
                y = work_area_y + SONIOX_LIVE_PREVIEW_TOP_OFFSET;
            }
            SonioxLivePreviewPosition::Bottom => {
                x = work_area_x + (work_area_width - width) / 2.0;
                y = work_area_y + work_area_height - height - SONIOX_LIVE_PREVIEW_BOTTOM_OFFSET;
            }
            SonioxLivePreviewPosition::NearCursor => {
                let (cursor_x, cursor_y) = input::get_cursor_position(app_handle)
                    .unwrap_or((
                        (work_area.position.x + (work_area.size.width as i32 / 2)),
                        (work_area.position.y + (work_area.size.height as i32 / 2)),
                    ));

                let cursor_x_logical = cursor_x as f64 / scale;
                let cursor_y_logical = cursor_y as f64 / scale;
                let distance = app_settings.soniox_live_preview_cursor_offset_px as f64;

                let min_x = work_area_x + SONIOX_LIVE_PREVIEW_CURSOR_EDGE_MARGIN;
                let max_x = work_area_x + work_area_width - width - SONIOX_LIVE_PREVIEW_CURSOR_EDGE_MARGIN;
                let min_y = work_area_y + SONIOX_LIVE_PREVIEW_CURSOR_EDGE_MARGIN;
                let max_y = work_area_y + work_area_height - height - SONIOX_LIVE_PREVIEW_CURSOR_EDGE_MARGIN;

                x = clamp_f64(cursor_x_logical - (width / 2.0), min_x, max_x);
                y = clamp_f64(cursor_y_logical - height - distance, min_y, max_y);
            }
            SonioxLivePreviewPosition::CustomXY => {
                x = app_settings.soniox_live_preview_custom_x_px as f64;
                y = app_settings.soniox_live_preview_custom_y_px as f64;
            }
        }

        return Some((x, y, width, height));
    }
    None
}

/// Creates the recording overlay window and keeps it hidden by default
#[cfg(not(target_os = "macos"))]
pub fn create_recording_overlay(app_handle: &AppHandle) {
    if let Some((x, y)) = calculate_overlay_position(app_handle) {
        match WebviewWindowBuilder::new(
            app_handle,
            "recording_overlay",
            tauri::WebviewUrl::App("src/overlay/index.html".into()),
        )
        .title("Recording")
        .position(x, y)
        .resizable(false)
        .inner_size(OVERLAY_WIDTH, OVERLAY_HEIGHT)
        .shadow(false)
        .maximizable(false)
        .minimizable(false)
        .closable(false)
        .accept_first_mouse(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .transparent(true)
        .focused(false)
        .visible(false)
        .build()
        {
            Ok(_window) => {
                debug!("Recording overlay window created successfully (hidden)");
            }
            Err(e) => {
                debug!("Failed to create recording overlay window: {}", e);
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub fn create_soniox_live_preview_window(app_handle: &AppHandle) {
    if app_handle
        .get_webview_window(SONIOX_LIVE_PREVIEW_WINDOW_LABEL)
        .is_some()
    {
        return;
    }

    if let Some((x, y, width, height)) = resolve_soniox_live_preview_geometry(app_handle) {
        match WebviewWindowBuilder::new(
            app_handle,
            SONIOX_LIVE_PREVIEW_WINDOW_LABEL,
            tauri::WebviewUrl::App("src/soniox-live-preview/index.html".into()),
        )
        .title("Live Preview")
        .position(x, y)
        .resizable(false)
        .inner_size(width, height)
        .maximizable(false)
        .minimizable(false)
        .closable(false)
        .accept_first_mouse(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .transparent(true)
        .focused(false)
        .visible(false)
        .build()
        {
            Ok(_window) => {
                debug!("Soniox live preview window created successfully (hidden)");
            }
            Err(e) => {
                debug!("Failed to create Soniox live preview window: {}", e);
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn create_soniox_live_preview_window(_app_handle: &AppHandle) {}

/// Creates the recording overlay panel and keeps it hidden by default (macOS)
#[cfg(target_os = "macos")]
pub fn create_recording_overlay(app_handle: &AppHandle) {
    if let Some((x, y)) = calculate_overlay_position(app_handle) {
        // PanelBuilder creates a Tauri window then converts it to NSPanel.
        // The window remains registered, so get_webview_window() still works.
        match PanelBuilder::<_, RecordingOverlayPanel>::new(app_handle, "recording_overlay")
            .url(WebviewUrl::App("src/overlay/index.html".into()))
            .title("Recording")
            .position(tauri::Position::Logical(tauri::LogicalPosition { x, y }))
            .level(PanelLevel::Status)
            .size(tauri::Size::Logical(tauri::LogicalSize {
                width: OVERLAY_WIDTH,
                height: OVERLAY_HEIGHT,
            }))
            .has_shadow(false)
            .transparent(true)
            .no_activate(true)
            .corner_radius(0.0)
            .with_window(|w| w.decorations(false).transparent(true))
            .collection_behavior(
                CollectionBehavior::new()
                    .can_join_all_spaces()
                    .full_screen_auxiliary(),
            )
            .build()
        {
            Ok(panel) => {
                let _ = panel.hide();
            }
            Err(e) => {
                log::error!("Failed to create recording overlay panel: {}", e);
            }
        }
    }
}

/// Shows the recording overlay window with fade-in animation
pub fn show_recording_overlay(app_handle: &AppHandle) {
    // Cancel pending error auto-hide timers so a new active overlay is not hidden.
    plus_overlay_state::invalidate_error_overlay_auto_hide();

    // Cancel any pending profile switch overlay auto-hide timer
    // by incrementing the generation counter
    PROFILE_OVERLAY_GENERATION.fetch_add(1, Ordering::SeqCst);

    // Check if overlay should be shown based on position setting
    let settings = settings::get_settings(app_handle);
    if settings.overlay_position == OverlayPosition::None {
        return;
    }

    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        // Update position before showing to prevent flicker from position changes
        if let Some((x, y)) = calculate_overlay_position(app_handle) {
            let _ = overlay_window
                .set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        }

        let _ = overlay_window.show();

        // On Windows, aggressively re-assert "topmost" in the native Z-order after showing
        #[cfg(target_os = "windows")]
        force_overlay_topmost(&overlay_window);

        // Emit event to trigger fade-in animation with recording state
        let payload = build_overlay_state_payload("recording", &settings);
        let _ = overlay_window.emit("show-overlay", payload);
    }
}

/// Shows the transcribing overlay window
pub fn show_transcribing_overlay(app_handle: &AppHandle) {
    // Cancel pending error auto-hide timers so a new active overlay is not hidden.
    plus_overlay_state::invalidate_error_overlay_auto_hide();

    // Check if overlay should be shown based on position setting
    let settings = settings::get_settings(app_handle);
    if settings.overlay_position == OverlayPosition::None {
        return;
    }

    update_overlay_position(app_handle);

    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        let _ = overlay_window.show();

        // On Windows, aggressively re-assert "topmost" in the native Z-order after showing
        #[cfg(target_os = "windows")]
        force_overlay_topmost(&overlay_window);

        // Emit event to switch to transcribing state
        let payload = build_overlay_state_payload("transcribing", &settings);
        let _ = overlay_window.emit("show-overlay", payload);
    }
}

/// Shows the sending overlay window (for remote API calls)
pub fn show_sending_overlay(app_handle: &AppHandle) {
    // Cancel pending error auto-hide timers so a new active overlay is not hidden.
    plus_overlay_state::invalidate_error_overlay_auto_hide();

    // Check if overlay should be shown based on position setting
    let settings = settings::get_settings(app_handle);
    if settings.overlay_position == OverlayPosition::None {
        return;
    }

    update_overlay_position(app_handle);

    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        let _ = overlay_window.show();

        // On Windows, aggressively re-assert "topmost" in the native Z-order after showing
        #[cfg(target_os = "windows")]
        force_overlay_topmost(&overlay_window);

        // Emit event to switch to sending state
        let payload = build_overlay_state_payload("sending", &settings);
        let _ = overlay_window.emit("show-overlay", payload);
    }
}

/// Shows the thinking overlay window (for LLM processing)
pub fn show_thinking_overlay(app_handle: &AppHandle) {
    // Cancel pending error auto-hide timers so a new active overlay is not hidden.
    plus_overlay_state::invalidate_error_overlay_auto_hide();

    // Check if overlay should be shown based on position setting
    let settings = settings::get_settings(app_handle);
    if settings.overlay_position == OverlayPosition::None {
        return;
    }

    update_overlay_position(app_handle);

    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        let _ = overlay_window.show();

        // On Windows, aggressively re-assert "topmost" in the native Z-order after showing
        #[cfg(target_os = "windows")]
        force_overlay_topmost(&overlay_window);

        // Emit event to switch to thinking state
        let payload = build_overlay_state_payload("thinking", &settings);
        let _ = overlay_window.emit("show-overlay", payload);
    }
}

/// Shows the finalizing overlay window (for Soniox live stop/finalization)
pub fn show_finalizing_overlay(app_handle: &AppHandle) {
    // Cancel pending error auto-hide timers so a new active overlay is not hidden.
    plus_overlay_state::invalidate_error_overlay_auto_hide();

    // Check if overlay should be shown based on position setting
    let settings = settings::get_settings(app_handle);
    if settings.overlay_position == OverlayPosition::None {
        return;
    }

    update_overlay_position(app_handle);

    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        let _ = overlay_window.show();

        // On Windows, aggressively re-assert "topmost" in the native Z-order after showing
        #[cfg(target_os = "windows")]
        force_overlay_topmost(&overlay_window);

        // Emit event to switch to finalizing state
        let payload = build_overlay_state_payload("finalizing", &settings);
        let _ = overlay_window.emit("show-overlay", payload);
    }
}

/// Updates the overlay window position based on current settings
pub fn update_overlay_position(app_handle: &AppHandle) {
    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        if let Some((x, y)) = calculate_overlay_position(app_handle) {
            let _ = overlay_window
                .set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        }
    }
}

/// Hides the recording overlay window with fade-out animation
pub fn hide_recording_overlay(app_handle: &AppHandle) {
    // Always hide the overlay regardless of settings - if setting was changed while recording,
    // we still want to hide it properly
    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        // Emit event to trigger fade-out animation
        let _ = overlay_window.emit("hide-overlay", ());
        // Hide immediately for faster stop/finalization response.
        let _ = overlay_window.hide();
    }
}

/// Immediately hides the recording overlay window (no animation delay).
///
/// Useful when the next operation is a screen capture, so we don't accidentally capture the overlay.
pub fn hide_recording_overlay_immediately(app_handle: &AppHandle) {
    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        let _ = overlay_window.hide();
    }
}

pub fn begin_soniox_live_preview_session() {
    if let Ok(mut runtime_state) = SONIOX_LIVE_PREVIEW_RUNTIME_STATE.lock() {
        runtime_state.mode = SonioxLivePreviewMode::Live;
        runtime_state.live_active = true;
    }
}

pub fn end_soniox_live_preview_session() {
    if let Ok(mut runtime_state) = SONIOX_LIVE_PREVIEW_RUNTIME_STATE.lock() {
        runtime_state.mode = SonioxLivePreviewMode::UiDemo;
        runtime_state.live_active = false;
    }
}

fn is_soniox_live_preview_session_active() -> bool {
    SONIOX_LIVE_PREVIEW_RUNTIME_STATE
        .lock()
        .map(|runtime_state| {
            runtime_state.live_active && runtime_state.mode == SonioxLivePreviewMode::Live
        })
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
pub fn show_soniox_live_preview_window(app_handle: &AppHandle) {
    let app_settings = settings::get_settings(app_handle);
    let preview_output_mode_active = crate::managers::preview_output_mode::is_active();
    if !app_settings.soniox_live_preview_enabled && !preview_output_mode_active {
        return;
    }

    if app_handle
        .get_webview_window(SONIOX_LIVE_PREVIEW_WINDOW_LABEL)
        .is_none()
    {
        create_soniox_live_preview_window(app_handle);
    }

    if let Some(window) = app_handle.get_webview_window(SONIOX_LIVE_PREVIEW_WINDOW_LABEL) {
        if let Some((x, y, width, height)) = resolve_soniox_live_preview_geometry(app_handle) {
            let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }));
            let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        }
        let _ = window.unminimize();
        let _ = window.show();
        force_overlay_topmost(&window);
        emit_soniox_live_preview_appearance_update(app_handle);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn show_soniox_live_preview_window(_app_handle: &AppHandle) {}

#[cfg(target_os = "windows")]
pub fn hide_soniox_live_preview_window(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window(SONIOX_LIVE_PREVIEW_WINDOW_LABEL) {
        let _ = window.hide();
    }
}

#[cfg(not(target_os = "windows"))]
pub fn hide_soniox_live_preview_window(_app_handle: &AppHandle) {}

#[cfg(target_os = "windows")]
pub fn reset_soniox_live_preview(app_handle: &AppHandle) {
    if let Ok(mut state) = SONIOX_LIVE_PREVIEW_STATE.lock() {
        state.final_text.clear();
        state.interim_text.clear();
    }
    let _ = app_handle.emit("soniox-live-preview-reset", ());
    let _ = app_handle.emit("soniox_live_preview_reset", ());
    if let Some(window) = app_handle.get_webview_window(SONIOX_LIVE_PREVIEW_WINDOW_LABEL) {
        let _ = window.emit("soniox-live-preview-reset", ());
        let _ = window.emit("soniox_live_preview_reset", ());
    }
}

#[cfg(not(target_os = "windows"))]
pub fn reset_soniox_live_preview(_app_handle: &AppHandle) {}

#[cfg(target_os = "windows")]
fn emit_soniox_live_preview_update_internal(
    app_handle: &AppHandle,
    final_text: &str,
    interim_text: &str,
) {
    if let Ok(mut state) = SONIOX_LIVE_PREVIEW_STATE.lock() {
        state.final_text.clear();
        state.final_text.push_str(final_text);
        state.interim_text.clear();
        state.interim_text.push_str(interim_text);
    }

    let payload = SonioxLivePreviewPayload {
        final_text: final_text.to_string(),
        interim_text: interim_text.to_string(),
    };

    let _ = app_handle.emit("soniox-live-preview-update", payload.clone());
    let _ = app_handle.emit("soniox_live_preview_update", payload.clone());

    if let Some(window) = app_handle.get_webview_window(SONIOX_LIVE_PREVIEW_WINDOW_LABEL) {
        let _ = window.emit("soniox-live-preview-update", payload.clone());
        let _ = window.emit("soniox_live_preview_update", payload.clone());
    }
}

#[cfg(target_os = "windows")]
pub fn emit_soniox_live_preview_update(
    app_handle: &AppHandle,
    final_text: &str,
    interim_text: &str,
) {
    if !is_soniox_live_preview_session_active() {
        return;
    }
    emit_soniox_live_preview_update_internal(app_handle, final_text, interim_text);
}

#[cfg(target_os = "windows")]
fn emit_soniox_live_preview_demo_update(
    app_handle: &AppHandle,
    final_text: &str,
    interim_text: &str,
) {
    if is_soniox_live_preview_session_active() {
        return;
    }
    emit_soniox_live_preview_update_internal(app_handle, final_text, interim_text);
}

#[cfg(target_os = "windows")]
pub fn emit_soniox_live_preview_appearance_update(app_handle: &AppHandle) {
    let payload = build_soniox_live_preview_appearance_payload(app_handle);

    let _ = app_handle.emit("soniox-live-preview-appearance-update", payload.clone());
    let _ = app_handle.emit("soniox_live_preview_appearance_update", payload.clone());

    if let Some(window) = app_handle.get_webview_window(SONIOX_LIVE_PREVIEW_WINDOW_LABEL) {
        let _ = window.emit("soniox-live-preview-appearance-update", payload.clone());
        let _ = window.emit("soniox_live_preview_appearance_update", payload.clone());
    }
}

#[cfg(not(target_os = "windows"))]
pub fn emit_soniox_live_preview_appearance_update(_app_handle: &AppHandle) {}

#[cfg(target_os = "windows")]
pub fn update_soniox_live_preview_window(app_handle: &AppHandle) {
    let app_settings = settings::get_settings(app_handle);
    let should_show =
        app_settings.soniox_live_preview_enabled || crate::managers::preview_output_mode::is_active();
    if let Some(window) = app_handle.get_webview_window(SONIOX_LIVE_PREVIEW_WINDOW_LABEL) {
        if !should_show {
            let _ = window.hide();
        } else if let Some((x, y, width, height)) = resolve_soniox_live_preview_geometry(app_handle) {
            let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }));
            let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        }
    }

    emit_soniox_live_preview_appearance_update(app_handle);
}

#[cfg(not(target_os = "windows"))]
pub fn update_soniox_live_preview_window(_app_handle: &AppHandle) {}

#[tauri::command]
#[specta::specta]
pub fn get_soniox_live_preview_state() -> SonioxLivePreviewPayload {
    SONIOX_LIVE_PREVIEW_STATE
        .lock()
        .map(|state| state.clone())
        .unwrap_or_default()
}

#[tauri::command]
#[specta::specta]
pub fn get_soniox_live_preview_appearance(
    app_handle: AppHandle,
) -> SonioxLivePreviewAppearancePayload {
    build_soniox_live_preview_appearance_payload(&app_handle)
}

#[tauri::command]
#[specta::specta]
pub fn get_preview_output_mode_state() -> PreviewOutputModeStatePayload {
    crate::managers::preview_output_mode::get_state_payload()
}

#[tauri::command]
#[specta::specta]
pub fn preview_soniox_live_preview_window(app_handle: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if is_soniox_live_preview_session_active() {
            // Live capture preview has strict priority over UI demo preview.
            return Ok(());
        }

        if let Ok(mut runtime_state) = SONIOX_LIVE_PREVIEW_RUNTIME_STATE.lock() {
            runtime_state.mode = SonioxLivePreviewMode::UiDemo;
        }

        if app_handle
            .get_webview_window(SONIOX_LIVE_PREVIEW_WINDOW_LABEL)
            .is_none()
        {
            create_soniox_live_preview_window(&app_handle);
        }

        if let Some(window) = app_handle.get_webview_window(SONIOX_LIVE_PREVIEW_WINDOW_LABEL) {
            if let Some((x, y, width, height)) = resolve_soniox_live_preview_geometry(&app_handle) {
                let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }));
                let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
            }
            emit_soniox_live_preview_appearance_update(&app_handle);
            let _ = window.unminimize();
            let _ = window.show();
            force_overlay_topmost(&window);
            emit_soniox_live_preview_demo_update(
                &app_handle,
                "Confirmed Text: The quick brown fox jumps over the lazy dog. ",
                "Live Draft: this part may still change before confirmation...",
            );
            let _ = window.set_focus();

            return Ok(());
        }

        return Err("Failed to open Soniox live preview window.".to_string());
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Soniox live preview is available on Windows only.".to_string())
    }
}

#[cfg(not(target_os = "windows"))]
pub fn emit_soniox_live_preview_update(
    _app_handle: &AppHandle,
    _final_text: &str,
    _interim_text: &str,
) {
}

pub fn emit_levels(app_handle: &AppHandle, levels: &Vec<f32>) {
    // emit levels to main app
    let _ = app_handle.emit("mic-level", levels);

    // also emit to the recording overlay if it's open
    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        let _ = overlay_window.emit("mic-level", levels);
    }
}

// ============================================================================
// Command Confirmation Overlay (Voice Command Center)
// ============================================================================

/// Calculates centered position for command confirmation overlay
fn calculate_command_confirm_position(app_handle: &AppHandle) -> Option<(f64, f64)> {
    if let Some(monitor) = get_monitor_with_cursor(app_handle) {
        let work_area = monitor.work_area();
        let scale = monitor.scale_factor();
        let work_area_width = work_area.size.width as f64 / scale;
        let work_area_height = work_area.size.height as f64 / scale;
        let work_area_x = work_area.position.x as f64 / scale;
        let work_area_y = work_area.position.y as f64 / scale;

        // Center the overlay
        let x = work_area_x + (work_area_width - COMMAND_CONFIRM_WIDTH) / 2.0;
        let y = work_area_y + (work_area_height - COMMAND_CONFIRM_HEIGHT) / 2.0 - 50.0; // Slightly above center

        return Some((x, y));
    }
    None
}

/// Calculates bottom-center position for the floating voice activation button window.
fn calculate_voice_button_position(app_handle: &AppHandle) -> Option<(f64, f64)> {
    if let Some(monitor) = get_monitor_with_cursor(app_handle) {
        let work_area = monitor.work_area();
        let scale = monitor.scale_factor();
        let work_area_width = work_area.size.width as f64 / scale;
        let work_area_height = work_area.size.height as f64 / scale;
        let work_area_x = work_area.position.x as f64 / scale;
        let work_area_y = work_area.position.y as f64 / scale;

        let x = work_area_x + (work_area_width - VOICE_BUTTON_WIDTH) / 2.0;
        let y = work_area_y + work_area_height - VOICE_BUTTON_HEIGHT - 40.0;
        return Some((x, y));
    }
    None
}

/// Shows the floating voice activation button window.
/// Creates the window if it doesn't exist yet.
#[cfg(target_os = "windows")]
pub fn show_voice_activation_button_window(app_handle: &AppHandle) -> Result<(), String> {
    let window_label = "voice_activation_button";
    info!("show_voice_activation_button_window called");
    let initial_position = calculate_voice_button_position(app_handle);

    // Track whether this is a new window (need to set initial position)
    let is_new_window;

    let window = if let Some(existing) = app_handle.get_webview_window(window_label) {
        info!("Reusing existing voice activation button window");
        is_new_window = false;
        existing
    } else if let Some((x, y)) = initial_position {
        is_new_window = true;
        info!(
            "Creating new voice activation button window at ({}, {})",
            x, y
        );
        match WebviewWindowBuilder::new(
            app_handle,
            window_label,
            tauri::WebviewUrl::App("src/voice-activation-button/index.html".into()),
        )
        .title("Voice Activation Button")
        .position(x, y)
        .inner_size(VOICE_BUTTON_WIDTH, VOICE_BUTTON_HEIGHT)
        .resizable(true)
        .maximizable(false)
        .minimizable(false)
        .closable(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .transparent(false)
        .focused(false)
        .visible(false)
        .build()
        {
            Ok(window) => window,
            Err(e) => {
                let msg = format!("Failed to create voice activation button window: {}", e);
                log::error!("{}", msg);
                return Err(msg);
            }
        }
    } else {
        is_new_window = true;
        info!("Primary position unavailable; using fallback position (100, 100)");
        // Fallback if monitor detection fails.
        match WebviewWindowBuilder::new(
            app_handle,
            window_label,
            tauri::WebviewUrl::App("src/voice-activation-button/index.html".into()),
        )
        .title("Voice Activation Button")
        .position(100.0, 100.0)
        .inner_size(VOICE_BUTTON_WIDTH, VOICE_BUTTON_HEIGHT)
        .resizable(true)
        .maximizable(false)
        .minimizable(false)
        .closable(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .transparent(false)
        .focused(false)
        .visible(false)
        .build()
        {
            Ok(window) => window,
            Err(e) => {
                let msg = format!(
                    "Could not calculate position and fallback window creation failed: {}",
                    e
                );
                log::error!("{}", msg);
                return Err(msg);
            }
        }
    };

    // Only set position for new windows - preserve user's drag position for existing ones
    if is_new_window {
        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: VOICE_BUTTON_WIDTH,
            height: VOICE_BUTTON_HEIGHT,
        }));
    }

    if let Err(e) = window.show() {
        let msg = format!("Failed to show voice activation button window: {}", e);
        log::error!("{}", msg);
        return Err(msg);
    }
    info!("Voice activation button window shown");
    force_overlay_topmost(&window);
    let _ = window.set_focus();
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn show_voice_activation_button_window(_app_handle: &AppHandle) -> Result<(), String> {
    Err("Voice activation button window is currently Windows-only.".to_string())
}
/// Shows the command confirmation overlay with the given payload.
/// Creates the window if it doesn't exist yet.
#[cfg(target_os = "windows")]
pub fn show_command_confirm_overlay(
    app_handle: &AppHandle,
    payload: crate::actions::CommandConfirmPayload,
) {
    use log::debug;

    let window_label = "command_confirm";

    debug!("show_command_confirm_overlay called");

    // Track whether we're creating a new window (need to wait longer for React to mount)
    let is_new_window;

    // Get or create the window
    let window = if let Some(existing) = app_handle.get_webview_window(window_label) {
        is_new_window = false;
        debug!("Reusing existing command_confirm window");
        existing
    } else {
        is_new_window = true;
        debug!("Creating new command_confirm window");
        // Create the window
        if let Some((x, y)) = calculate_command_confirm_position(app_handle) {
            debug!("Window position calculated: ({}, {})", x, y);
            match WebviewWindowBuilder::new(
                app_handle,
                window_label,
                tauri::WebviewUrl::App("src/command-confirm/index.html".into()),
            )
            .title("Voice Command")
            .position(x, y)
            .inner_size(COMMAND_CONFIRM_WIDTH, COMMAND_CONFIRM_HEIGHT)
            .resizable(true) // Allow programmatic resizing for error display
            .maximizable(false)
            .minimizable(false)
            .closable(true)
            .decorations(false)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .transparent(true)
            .focused(true)
            .visible(false)
            .build()
            {
                Ok(window) => {
                    debug!("Command confirm overlay window created successfully");
                    window
                }
                Err(e) => {
                    log::error!("Failed to create command confirm window: {}", e);
                    return;
                }
            }
        } else {
            log::error!("Could not calculate position for command confirm overlay");
            return;
        }
    };

    // Update position
    if let Some((x, y)) = calculate_command_confirm_position(app_handle) {
        let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
    }

    // For new windows, we need to wait for the webview to load before emitting the payload.
    // For existing windows, emit the payload immediately, then show.
    if is_new_window {
        // New window: wait for webview to load, THEN emit payload, THEN show.
        // Emitting before webview loads will lose the event!
        let window_clone = window.clone();
        let payload_clone = payload.clone();
        std::thread::spawn(move || {
            // Wait for webview to load and React to mount
            std::thread::sleep(std::time::Duration::from_millis(200));
            // Now emit the payload (React is ready to receive it)
            if let Err(e) = window_clone.emit("show-command-confirm", payload_clone) {
                log::error!("Failed to emit show-command-confirm event: {}", e);
            }
            // Small delay for React to process the payload
            std::thread::sleep(std::time::Duration::from_millis(50));
            // Show the window (now with content)
            if let Err(e) = window_clone.show() {
                log::error!("Failed to show window: {}", e);
            }
            force_overlay_topmost(&window_clone);
            let _ = window_clone.set_focus();
        });
    } else {
        // Existing window: emit payload first, then show immediately
        if let Err(e) = window.emit("show-command-confirm", payload) {
            log::error!("Failed to emit show-command-confirm event: {}", e);
        }

        // Small delay to let React process the payload before showing
        let window_clone = window.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(30));
            if let Err(e) = window_clone.show() {
                log::error!("Failed to show window: {}", e);
            }
            force_overlay_topmost(&window_clone);
            let _ = window_clone.set_focus();
        });
    }
}

// ============================================================================
// Profile Switch Overlay (Transcription Profiles)
// ============================================================================

/// Shows a brief overlay notification when switching transcription profiles.
/// Uses the existing recording overlay to display the profile name, then auto-hides.
pub fn show_profile_switch_overlay(app_handle: &AppHandle, profile_name: &str) {
    // Cancel pending error auto-hide timers so a new active overlay is not hidden.
    plus_overlay_state::invalidate_error_overlay_auto_hide();

    let settings = settings::get_settings(app_handle);
    if settings.overlay_position == OverlayPosition::None {
        return;
    }

    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        // Update position
        if let Some((x, y)) = calculate_overlay_position(app_handle) {
            let _ = overlay_window
                .set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        }

        let _ = overlay_window.show();

        #[cfg(target_os = "windows")]
        force_overlay_topmost(&overlay_window);

        // Emit profile name for display
        let _ = overlay_window.emit("show-profile-switch", profile_name);

        // Capture the current generation before spawning the timer thread.
        // If a recording starts before the timer fires, the generation will change
        // and we'll skip hiding the overlay.
        let generation_at_start = PROFILE_OVERLAY_GENERATION.load(Ordering::SeqCst);

        // Auto-hide after a short delay (unless a recording has started)
        let window_clone = overlay_window.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1500));

            // Check if generation changed (recording started) - if so, don't hide
            if PROFILE_OVERLAY_GENERATION.load(Ordering::SeqCst) != generation_at_start {
                return;
            }

            let _ = window_clone.emit("hide-overlay", ());
            std::thread::sleep(std::time::Duration::from_millis(300));

            // Check again before actually hiding the window
            if PROFILE_OVERLAY_GENERATION.load(Ordering::SeqCst) != generation_at_start {
                return;
            }

            let _ = window_clone.hide();
        });
    }
}
