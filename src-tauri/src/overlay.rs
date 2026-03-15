use crate::input;
use crate::managers::preview_output_mode::PreviewOutputModeStatePayload;
use crate::plus_overlay_state;
use crate::settings;
use crate::settings::{
    OverlayPosition, RecordingOverlayAnimatedBorderMode, RecordingOverlayBackgroundMode,
    RecordingOverlayBarStyle, RecordingOverlayCenterpieceMode, RecordingOverlayMaterialMode,
    RecordingOverlayTheme, SonioxLivePreviewPosition, SonioxLivePreviewSize, SonioxLivePreviewTheme,
};
use serde::Serialize;
use specta::Type;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{LazyLock, Mutex};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};

/// Counter used to cancel pending transient message overlay auto-hide timers.
/// Each time a recording overlay or another transient message overlay is shown,
/// this is incremented so stale timers do not hide newer overlays.
static TRANSIENT_OVERLAY_GENERATION: AtomicU64 = AtomicU64::new(0);
static RECORDING_OVERLAY_LAYOUT: AtomicU8 = AtomicU8::new(0);

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

const OVERLAY_HEIGHT: f64 = 36.0;
const ERROR_OVERLAY_WIDTH: f64 = 340.0;
const ERROR_OVERLAY_HEIGHT: f64 = 82.0;
const RECORDING_OVERLAY_BAR_GAP: f64 = 3.0;

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

#[derive(Serialize, Clone)]
struct TransientMessageOverlayPayload {
    state: String,
    message: String,
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
    pub close_hotkey: String,
    pub clear_hotkey: String,
    pub flush_hotkey: String,
    pub process_hotkey: String,
    pub insert_hotkey: String,
    pub delete_until_dot_or_comma_hotkey: String,
    pub delete_until_dot_hotkey: String,
    pub delete_last_word_hotkey: String,
    pub show_clear_button: bool,
    pub show_flush_button: bool,
    pub show_process_button: bool,
    pub show_insert_button: bool,
    pub show_delete_until_dot_or_comma_button: bool,
    pub show_delete_until_dot_button: bool,
    pub show_delete_last_word_button: bool,
    pub ctrl_backspace_delete_last_word: bool,
    pub backspace_delete_last_char: bool,
    pub show_drag_grip: bool,
}

#[derive(Serialize, Clone, Type)]
pub struct RecordingOverlayAppearancePayload {
    custom_enabled: bool,
    theme: String,
    background_mode: String,
    material_mode: String,
    centerpiece_mode: String,
    animated_border_mode: String,
    accent_color: String,
    show_status_icon: bool,
    bar_count: u8,
    bar_width_px: u8,
    bar_style: String,
    show_drag_grip: bool,
    audio_reactive_scale: bool,
    audio_reactive_scale_max_percent: u8,
    voice_sensitivity_percent: u8,
    animation_softness_percent: u8,
    depth_parallax_percent: u8,
    opacity_percent: u8,
    silence_fade: bool,
    silence_opacity_percent: u8,
    frame_width_px: u16,
    frame_height_px: u16,
}

static SONIOX_LIVE_PREVIEW_STATE: LazyLock<Mutex<SonioxLivePreviewPayload>> =
    LazyLock::new(|| Mutex::new(SonioxLivePreviewPayload::default()));
static SONIOX_LIVE_PREVIEW_RUNTIME_STATE: LazyLock<Mutex<SonioxLivePreviewRuntimeState>> =
    LazyLock::new(|| Mutex::new(SonioxLivePreviewRuntimeState::default()));

#[derive(Clone, Copy)]
struct RecordingOverlayWindowMetrics {
    frame_width: f64,
    frame_height: f64,
    padding: f64,
    window_width: f64,
    window_height: f64,
}

fn build_overlay_state_payload(
    state: &str,
    settings: &settings::AppSettings,
) -> OverlayStatePayload {
    let indicator = crate::text_replacement_decapitalize::indicator_state(
        settings.text_replacement_decapitalize_after_edit_key_enabled,
    );
    OverlayStatePayload {
        state: state.to_string(),
        decapitalize_eligible: indicator.eligible,
        decapitalize_armed: indicator.armed,
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
                // Tauri monitor coordinates are physical pixels, while cursor coordinates
                // may be logical depending on DPI-awareness. Normalize monitors to logical.
                let scale = monitor.scale_factor();
                let position = PhysicalPosition::new(
                    (monitor.position().x as f64 / scale) as i32,
                    (monitor.position().y as f64 / scale) as i32,
                );
                let size = PhysicalSize::new(
                    (monitor.size().width as f64 / scale) as u32,
                    (monitor.size().height as f64 / scale) as u32,
                );
                if is_mouse_within_monitor(mouse_location, &position, &size) {
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

#[derive(Clone, Copy)]
struct LogicalBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn get_monitor_logical_bounds(monitor: &tauri::Monitor) -> LogicalBounds {
    let scale = monitor.scale_factor();
    LogicalBounds {
        x: monitor.position().x as f64 / scale,
        y: monitor.position().y as f64 / scale,
        width: monitor.size().width as f64 / scale,
        height: monitor.size().height as f64 / scale,
    }
}

#[cfg(target_os = "windows")]
fn get_monitor_logical_work_area_bounds(monitor: &tauri::Monitor) -> Option<LogicalBounds> {
    use std::mem::size_of;
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromRect, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    let monitor_rect = RECT {
        left: monitor.position().x,
        top: monitor.position().y,
        right: monitor.position().x + monitor.size().width as i32,
        bottom: monitor.position().y + monitor.size().height as i32,
    };

    unsafe {
        let hmonitor = MonitorFromRect(&monitor_rect, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(hmonitor, &mut info as *mut MONITORINFO).as_bool() {
            return None;
        }

        // Win32 reports rcWork in physical pixels. Convert it back to the same
        // logical coordinate space used for Tauri window placement.
        let scale = monitor.scale_factor();
        Some(LogicalBounds {
            x: info.rcWork.left as f64 / scale,
            y: info.rcWork.top as f64 / scale,
            width: (info.rcWork.right - info.rcWork.left) as f64 / scale,
            height: (info.rcWork.bottom - info.rcWork.top) as f64 / scale,
        })
    }
}

#[cfg(not(target_os = "windows"))]
fn get_monitor_logical_work_area_bounds(_monitor: &tauri::Monitor) -> Option<LogicalBounds> {
    None
}

fn get_monitor_logical_auto_position_bounds(
    monitor: &tauri::Monitor,
    allow_reserved_areas: bool,
) -> LogicalBounds {
    let full_bounds = get_monitor_logical_bounds(monitor);
    if allow_reserved_areas {
        return full_bounds;
    }

    get_monitor_logical_work_area_bounds(monitor).unwrap_or(full_bounds)
}

fn recording_overlay_theme_key(theme: RecordingOverlayTheme) -> &'static str {
    match theme {
        RecordingOverlayTheme::Classic => "classic",
        RecordingOverlayTheme::Minimal => "minimal",
        RecordingOverlayTheme::Glass => "glass",
    }
}

fn recording_overlay_background_mode_key(mode: RecordingOverlayBackgroundMode) -> &'static str {
    match mode {
        RecordingOverlayBackgroundMode::Mist => "mist",
        RecordingOverlayBackgroundMode::PetalsHaze => "petals_haze",
        RecordingOverlayBackgroundMode::SoftGlowField => "soft_glow_field",
        RecordingOverlayBackgroundMode::Stardust => "stardust",
        RecordingOverlayBackgroundMode::SilkFog => "silk_fog",
        RecordingOverlayBackgroundMode::FireflyVeil => "firefly_veil",
        RecordingOverlayBackgroundMode::RoseSparks => "rose_sparks",
        RecordingOverlayBackgroundMode::None => "none",
    }
}

fn recording_overlay_material_mode_key(mode: RecordingOverlayMaterialMode) -> &'static str {
    match mode {
        RecordingOverlayMaterialMode::LiquidGlass => "liquid_glass",
        RecordingOverlayMaterialMode::Pearl => "pearl",
        RecordingOverlayMaterialMode::VelvetNeon => "velvet_neon",
        RecordingOverlayMaterialMode::Frost => "frost",
        RecordingOverlayMaterialMode::CandyChrome => "candy_chrome",
    }
}

fn recording_overlay_centerpiece_mode_key(mode: RecordingOverlayCenterpieceMode) -> &'static str {
    match mode {
        RecordingOverlayCenterpieceMode::HaloCore => "halo_core",
        RecordingOverlayCenterpieceMode::AuroraRibbon => "aurora_ribbon",
        RecordingOverlayCenterpieceMode::OrbitalBeads => "orbital_beads",
        RecordingOverlayCenterpieceMode::BloomHeart => "bloom_heart",
        RecordingOverlayCenterpieceMode::SignalCrown => "signal_crown",
        RecordingOverlayCenterpieceMode::None => "none",
    }
}

fn recording_overlay_animated_border_mode_key(
    mode: RecordingOverlayAnimatedBorderMode,
) -> &'static str {
    match mode {
        RecordingOverlayAnimatedBorderMode::ShimmerEdge => "shimmer_edge",
        RecordingOverlayAnimatedBorderMode::TravelingHighlight => "traveling_highlight",
        RecordingOverlayAnimatedBorderMode::BreathingContour => "breathing_contour",
        RecordingOverlayAnimatedBorderMode::None => "none",
    }
}

fn recording_overlay_bar_style_key(style: RecordingOverlayBarStyle) -> &'static str {
    match style {
        RecordingOverlayBarStyle::Aurora => "aurora",
        RecordingOverlayBarStyle::BloomBounce => "bloom_bounce",
        RecordingOverlayBarStyle::Solid => "solid",
        RecordingOverlayBarStyle::Capsule => "capsule",
        RecordingOverlayBarStyle::Comet => "comet",
        RecordingOverlayBarStyle::Constellation => "constellation",
        RecordingOverlayBarStyle::Crown => "crown",
        RecordingOverlayBarStyle::Daisy => "daisy",
        RecordingOverlayBarStyle::Ember => "ember",
        RecordingOverlayBarStyle::Fireflies => "fireflies",
        RecordingOverlayBarStyle::GardenSway => "garden_sway",
        RecordingOverlayBarStyle::Glow => "glow",
        RecordingOverlayBarStyle::Hologram => "hologram",
        RecordingOverlayBarStyle::Helix => "helix",
        RecordingOverlayBarStyle::Lotus => "lotus",
        RecordingOverlayBarStyle::Matrix => "matrix",
        RecordingOverlayBarStyle::Morse => "morse",
        RecordingOverlayBarStyle::Petals => "petals",
        RecordingOverlayBarStyle::PetalRain => "petal_rain",
        RecordingOverlayBarStyle::Prism => "prism",
        RecordingOverlayBarStyle::PulseRings => "pulse_rings",
        RecordingOverlayBarStyle::Radar => "radar",
        RecordingOverlayBarStyle::Shards => "shards",
        RecordingOverlayBarStyle::Retro => "retro",
        RecordingOverlayBarStyle::Needles => "needles",
        RecordingOverlayBarStyle::Orbit => "orbit",
        RecordingOverlayBarStyle::Skyline => "skyline",
        RecordingOverlayBarStyle::Tuner => "tuner",
        RecordingOverlayBarStyle::Vinyl => "vinyl",
    }
}

fn build_recording_overlay_appearance_payload(
    app_handle: &AppHandle,
) -> RecordingOverlayAppearancePayload {
    let settings = settings::get_settings(app_handle);
    let metrics =
        recording_overlay_window_metrics(app_handle, current_recording_overlay_layout());
    RecordingOverlayAppearancePayload {
        custom_enabled: settings.recording_overlay_custom_enabled,
        theme: recording_overlay_theme_key(settings.recording_overlay_theme).to_string(),
        background_mode: recording_overlay_background_mode_key(
            settings.recording_overlay_background_mode,
        )
        .to_string(),
        material_mode: recording_overlay_material_mode_key(
            settings.recording_overlay_material_mode,
        )
        .to_string(),
        centerpiece_mode: recording_overlay_centerpiece_mode_key(
            settings.recording_overlay_centerpiece_mode,
        )
        .to_string(),
        animated_border_mode: recording_overlay_animated_border_mode_key(
            settings.recording_overlay_animated_border_mode,
        )
        .to_string(),
        accent_color: settings.recording_overlay_accent_color,
        show_status_icon: settings.recording_overlay_show_status_icon,
        bar_count: settings.recording_overlay_bar_count.clamp(3, 16),
        bar_width_px: settings.recording_overlay_bar_width_px.clamp(2, 12),
        bar_style: recording_overlay_bar_style_key(settings.recording_overlay_bar_style)
            .to_string(),
        show_drag_grip: settings.recording_overlay_show_drag_grip,
        audio_reactive_scale: settings.recording_overlay_audio_reactive_scale,
        audio_reactive_scale_max_percent: settings
            .recording_overlay_audio_reactive_scale_max_percent
            .clamp(0, 24),
        voice_sensitivity_percent: settings
            .recording_overlay_voice_sensitivity_percent
            .clamp(0, 100),
        animation_softness_percent: settings
            .recording_overlay_animation_softness_percent
            .clamp(0, 100),
        depth_parallax_percent: settings
            .recording_overlay_depth_parallax_percent
            .clamp(0, 100),
        opacity_percent: settings.recording_overlay_opacity_percent.clamp(20, 100),
        silence_fade: settings.recording_overlay_silence_fade,
        silence_opacity_percent: settings.recording_overlay_silence_opacity_percent.clamp(20, 100),
        frame_width_px: metrics.frame_width.round().clamp(0.0, u16::MAX as f64) as u16,
        frame_height_px: metrics.frame_height.round().clamp(0.0, u16::MAX as f64) as u16,
    }
}

fn emit_recording_overlay_appearance_update(app_handle: &AppHandle) {
    let payload = build_recording_overlay_appearance_payload(app_handle);

    let _ = app_handle.emit("recording-overlay-appearance-update", payload.clone());
    let _ = app_handle.emit("recording_overlay_appearance_update", payload.clone());

    if let Some(window) = app_handle.get_webview_window("recording_overlay") {
        let _ = window.emit("recording-overlay-appearance-update", payload.clone());
        let _ = window.emit("recording_overlay_appearance_update", payload);
    }
}

fn calculate_overlay_position_for_window(
    app_handle: &AppHandle,
    metrics: RecordingOverlayWindowMetrics,
) -> Option<(f64, f64)> {
    let monitor = get_monitor_with_cursor(app_handle)?;
    let settings = settings::get_settings(app_handle);
    let bounds = get_monitor_logical_auto_position_bounds(
        &monitor,
        settings.auto_position_allow_reserved_areas,
    );

    if settings.recording_overlay_use_manual_position {
        return Some((
            settings.recording_overlay_custom_x_px as f64 - metrics.padding,
            settings.recording_overlay_custom_y_px as f64 - metrics.padding,
        ));
    }

    let window_x = bounds.x + (bounds.width - metrics.window_width) / 2.0;
    let window_y = match settings.overlay_position {
        OverlayPosition::Top => bounds.y + OVERLAY_TOP_OFFSET,
        OverlayPosition::Bottom | OverlayPosition::None => {
            bounds.y + bounds.height - metrics.window_height - OVERLAY_BOTTOM_OFFSET
        }
    };

    Some((window_x, window_y))
}

fn apply_recording_overlay_layout(
    app_handle: &AppHandle,
    metrics: RecordingOverlayWindowMetrics,
) {
    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        let _ = overlay_window.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: metrics.window_width,
            height: metrics.window_height,
        }));
        if let Some((x, y)) = calculate_overlay_position_for_window(app_handle, metrics) {
            let _ = overlay_window
                .set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        }
        emit_recording_overlay_appearance_update(app_handle);
    }
}

fn current_recording_overlay_layout() -> RecordingOverlayLayout {
    match RECORDING_OVERLAY_LAYOUT.load(Ordering::SeqCst) {
        1 => RecordingOverlayLayout::Error,
        _ => RecordingOverlayLayout::Default,
    }
}

fn recording_overlay_default_width(app_handle: &AppHandle) -> f64 {
    let settings = settings::get_settings(app_handle);
    let bar_count = settings.recording_overlay_bar_count.clamp(3, 16) as f64;
    let minimum_width = settings.recording_overlay_width_px.clamp(172, 420) as f64;
    let base_bar_width = settings.recording_overlay_bar_width_px.clamp(2, 12) as f64;
    let bar_width = match settings.recording_overlay_bar_style {
        RecordingOverlayBarStyle::Vinyl => base_bar_width + 6.0,
        RecordingOverlayBarStyle::BloomBounce
        | RecordingOverlayBarStyle::Daisy
        | RecordingOverlayBarStyle::Lotus
        | RecordingOverlayBarStyle::GardenSway => base_bar_width + 10.0,
        RecordingOverlayBarStyle::Constellation
        | RecordingOverlayBarStyle::Fireflies
        | RecordingOverlayBarStyle::Helix
        | RecordingOverlayBarStyle::Petals
        | RecordingOverlayBarStyle::PetalRain
        | RecordingOverlayBarStyle::PulseRings => base_bar_width + 8.0,
        RecordingOverlayBarStyle::Orbit
        | RecordingOverlayBarStyle::Tuner
        | RecordingOverlayBarStyle::Morse => base_bar_width + 2.0,
        _ => base_bar_width,
    };
    let bar_gap_count = if bar_count > 1.0 { bar_count - 1.0 } else { 0.0 };
    let bar_track_width =
        (bar_count * bar_width) + (bar_gap_count * RECORDING_OVERLAY_BAR_GAP);
    let status_icon_width = if settings.recording_overlay_show_status_icon {
        28.0
    } else {
        0.0
    };

    (60.0 + status_icon_width + bar_track_width).max(minimum_width)
}

fn recording_overlay_frame_dimensions(
    app_handle: &AppHandle,
    layout: RecordingOverlayLayout,
) -> (f64, f64) {
    match layout {
        RecordingOverlayLayout::Default => {
            (recording_overlay_default_width(app_handle), OVERLAY_HEIGHT)
        }
        RecordingOverlayLayout::Error => (ERROR_OVERLAY_WIDTH, ERROR_OVERLAY_HEIGHT),
    }
}

fn recording_overlay_window_padding(
    app_handle: &AppHandle,
    layout: RecordingOverlayLayout,
    frame_width: f64,
    frame_height: f64,
) -> f64 {
    let settings = settings::get_settings(app_handle);
    let reactive_padding = if settings.recording_overlay_audio_reactive_scale {
        let boost = settings
            .recording_overlay_audio_reactive_scale_max_percent
            .clamp(0, 24) as f64
            / 100.0;
        frame_width.max(frame_height) * boost * 0.5
    } else {
        0.0
    };

    let style_padding = match settings.recording_overlay_bar_style {
        RecordingOverlayBarStyle::Aurora
        | RecordingOverlayBarStyle::Glow
        | RecordingOverlayBarStyle::Comet
        | RecordingOverlayBarStyle::Ember => 6.0,
        RecordingOverlayBarStyle::BloomBounce
        | RecordingOverlayBarStyle::Daisy
        | RecordingOverlayBarStyle::Lotus
        | RecordingOverlayBarStyle::GardenSway => 12.0,
        RecordingOverlayBarStyle::Constellation
        | RecordingOverlayBarStyle::Fireflies
        | RecordingOverlayBarStyle::Helix
        | RecordingOverlayBarStyle::Petals
        | RecordingOverlayBarStyle::PetalRain
        | RecordingOverlayBarStyle::PulseRings => 10.0,
        _ => 4.0,
    };

    let centerpiece_padding = match settings.recording_overlay_centerpiece_mode {
        RecordingOverlayCenterpieceMode::AuroraRibbon => 10.0,
        RecordingOverlayCenterpieceMode::OrbitalBeads
        | RecordingOverlayCenterpieceMode::HaloCore
        | RecordingOverlayCenterpieceMode::BloomHeart
        | RecordingOverlayCenterpieceMode::SignalCrown => 6.0,
        RecordingOverlayCenterpieceMode::None => 0.0,
    };

    let border_padding = match settings.recording_overlay_animated_border_mode {
        RecordingOverlayAnimatedBorderMode::TravelingHighlight => 6.0,
        RecordingOverlayAnimatedBorderMode::BreathingContour => 5.0,
        RecordingOverlayAnimatedBorderMode::ShimmerEdge => 4.0,
        RecordingOverlayAnimatedBorderMode::None => 0.0,
    };

    let ambient_padding = match settings.recording_overlay_background_mode {
        RecordingOverlayBackgroundMode::Mist
        | RecordingOverlayBackgroundMode::SoftGlowField
        | RecordingOverlayBackgroundMode::SilkFog => 6.0,
        RecordingOverlayBackgroundMode::PetalsHaze
        | RecordingOverlayBackgroundMode::Stardust
        | RecordingOverlayBackgroundMode::FireflyVeil
        | RecordingOverlayBackgroundMode::RoseSparks => 4.0,
        RecordingOverlayBackgroundMode::None => 0.0,
    };

    let material_padding = match settings.recording_overlay_material_mode {
        RecordingOverlayMaterialMode::VelvetNeon | RecordingOverlayMaterialMode::CandyChrome => 5.0,
        RecordingOverlayMaterialMode::LiquidGlass
        | RecordingOverlayMaterialMode::Pearl
        | RecordingOverlayMaterialMode::Frost => 3.0,
    };

    let parallax_padding =
        settings.recording_overlay_depth_parallax_percent.clamp(0, 100) as f64 * 0.08;

    let layout_padding = match layout {
        RecordingOverlayLayout::Error => 10.0,
        RecordingOverlayLayout::Default => 4.0,
    };

    // Keep extra transparent room around overlays that scale or glow so the
    // visible frame can grow inside the window without clipping.
    (reactive_padding
        .max(style_padding)
        .max(centerpiece_padding)
        .max(border_padding)
        .max(ambient_padding)
        .max(material_padding)
        + parallax_padding
        + layout_padding)
        .ceil()
}

fn recording_overlay_window_metrics(
    app_handle: &AppHandle,
    layout: RecordingOverlayLayout,
) -> RecordingOverlayWindowMetrics {
    let (frame_width, frame_height) = recording_overlay_frame_dimensions(app_handle, layout);
    let padding =
        recording_overlay_window_padding(app_handle, layout, frame_width, frame_height);

    RecordingOverlayWindowMetrics {
        frame_width,
        frame_height,
        padding,
        window_width: frame_width + (padding * 2.0),
        window_height: frame_height + (padding * 2.0),
    }
}

fn set_recording_overlay_layout(app_handle: &AppHandle, layout: RecordingOverlayLayout) {
    RECORDING_OVERLAY_LAYOUT.store(layout as u8, Ordering::SeqCst);
    let metrics = recording_overlay_window_metrics(app_handle, layout);
    apply_recording_overlay_layout(app_handle, metrics);
}

pub fn set_recording_overlay_default_layout(app_handle: &AppHandle) {
    set_recording_overlay_layout(app_handle, RecordingOverlayLayout::Default);
}

pub fn set_recording_overlay_error_layout(app_handle: &AppHandle) {
    set_recording_overlay_layout(app_handle, RecordingOverlayLayout::Error);
}

fn soniox_live_preview_dimensions(app_settings: &settings::AppSettings) -> (f64, f64) {
    match app_settings.soniox_live_preview_size {
        SonioxLivePreviewSize::Small => (
            SONIOX_LIVE_PREVIEW_SMALL_WIDTH,
            SONIOX_LIVE_PREVIEW_SMALL_HEIGHT,
        ),
        SonioxLivePreviewSize::Medium => (
            SONIOX_LIVE_PREVIEW_MEDIUM_WIDTH,
            SONIOX_LIVE_PREVIEW_MEDIUM_HEIGHT,
        ),
        SonioxLivePreviewSize::Large => (
            SONIOX_LIVE_PREVIEW_LARGE_WIDTH,
            SONIOX_LIVE_PREVIEW_LARGE_HEIGHT,
        ),
        SonioxLivePreviewSize::Custom => (
            app_settings.soniox_live_preview_custom_width_px.clamp(
                SONIOX_LIVE_PREVIEW_MIN_CUSTOM_WIDTH_PX,
                SONIOX_LIVE_PREVIEW_MAX_CUSTOM_WIDTH_PX,
            ) as f64,
            app_settings.soniox_live_preview_custom_height_px.clamp(
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
        opacity_percent: app_settings
            .soniox_live_preview_opacity_percent
            .clamp(35, 100),
        font_color: normalize_preview_color(
            &app_settings.soniox_live_preview_font_color,
            "#f5f5f5",
        ),
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
        close_hotkey: app_settings
            .soniox_live_preview_close_hotkey
            .trim()
            .to_string(),
        clear_hotkey: app_settings
            .soniox_live_preview_clear_hotkey
            .trim()
            .to_string(),
        flush_hotkey: app_settings
            .soniox_live_preview_flush_hotkey
            .trim()
            .to_string(),
        process_hotkey: app_settings
            .soniox_live_preview_process_hotkey
            .trim()
            .to_string(),
        insert_hotkey: app_settings
            .soniox_live_preview_insert_hotkey
            .trim()
            .to_string(),
        delete_until_dot_or_comma_hotkey: app_settings
            .soniox_live_preview_delete_until_dot_or_comma_hotkey
            .trim()
            .to_string(),
        delete_until_dot_hotkey: app_settings
            .soniox_live_preview_delete_until_dot_hotkey
            .trim()
            .to_string(),
        delete_last_word_hotkey: app_settings
            .soniox_live_preview_delete_last_word_hotkey
            .trim()
            .to_string(),
        show_clear_button: app_settings.soniox_live_preview_show_clear_button,
        show_flush_button: app_settings.soniox_live_preview_show_flush_button,
        show_process_button: app_settings.soniox_live_preview_show_process_button,
        show_insert_button: app_settings.soniox_live_preview_show_insert_button,
        show_delete_until_dot_or_comma_button: app_settings
            .soniox_live_preview_show_delete_until_dot_or_comma_button,
        show_delete_until_dot_button: app_settings.soniox_live_preview_show_delete_until_dot_button,
        show_delete_last_word_button: app_settings.soniox_live_preview_show_delete_last_word_button,
        ctrl_backspace_delete_last_word: app_settings
            .soniox_live_preview_ctrl_backspace_delete_last_word,
        backspace_delete_last_char: app_settings.soniox_live_preview_backspace_delete_last_char,
        show_drag_grip: app_settings.soniox_live_preview_show_drag_grip,
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
        let scale = monitor.scale_factor();
        let bounds = get_monitor_logical_auto_position_bounds(
            &monitor,
            app_settings.auto_position_allow_reserved_areas,
        );

        let x;
        let y;

        match app_settings.soniox_live_preview_position {
            SonioxLivePreviewPosition::Top => {
                x = bounds.x + (bounds.width - width) / 2.0;
                y = bounds.y + SONIOX_LIVE_PREVIEW_TOP_OFFSET;
            }
            SonioxLivePreviewPosition::Bottom => {
                x = bounds.x + (bounds.width - width) / 2.0;
                y = bounds.y + bounds.height - height - SONIOX_LIVE_PREVIEW_BOTTOM_OFFSET;
            }
            SonioxLivePreviewPosition::NearCursor => {
                let (cursor_x, cursor_y) = input::get_cursor_position(app_handle).unwrap_or((
                    monitor.position().x + (monitor.size().width as i32 / 2),
                    monitor.position().y + (monitor.size().height as i32 / 2),
                ));

                let cursor_x_logical = cursor_x as f64 / scale;
                let cursor_y_logical = cursor_y as f64 / scale;
                let distance = app_settings.soniox_live_preview_cursor_offset_px as f64;

                let min_x = bounds.x + SONIOX_LIVE_PREVIEW_CURSOR_EDGE_MARGIN;
                let max_x =
                    bounds.x + bounds.width - width - SONIOX_LIVE_PREVIEW_CURSOR_EDGE_MARGIN;
                let min_y = bounds.y + SONIOX_LIVE_PREVIEW_CURSOR_EDGE_MARGIN;
                let max_y =
                    bounds.y + bounds.height - height - SONIOX_LIVE_PREVIEW_CURSOR_EDGE_MARGIN;

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
    let metrics =
        recording_overlay_window_metrics(app_handle, current_recording_overlay_layout());
    let position = calculate_overlay_position_for_window(app_handle, metrics);

    #[cfg(not(target_os = "linux"))]
    if position.is_none() {
        debug!("Failed to determine overlay position, not creating overlay window");
        return;
    }

    let mut builder = WebviewWindowBuilder::new(
        app_handle,
        "recording_overlay",
        tauri::WebviewUrl::App("src/overlay/index.html".into()),
    )
    .title("Recording")
    .resizable(false)
    .inner_size(metrics.window_width, metrics.window_height)
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
    .visible(false);

    if let Some(data_dir) = crate::portable::data_dir() {
        builder = builder.data_directory(data_dir.join("webview"));
    }

    match builder.build()
    {
        Ok(_window) => {
            debug!("Recording overlay window created successfully (hidden)");
        }
        Err(e) => {
            debug!("Failed to create recording overlay window: {}", e);
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
        let mut builder = WebviewWindowBuilder::new(
            app_handle,
            SONIOX_LIVE_PREVIEW_WINDOW_LABEL,
            tauri::WebviewUrl::App("src/soniox-live-preview/index.html".into()),
        )
        .title("Live Preview")
        .position(x, y)
        .resizable(true)
        .inner_size(width, height)
        .min_inner_size(
            SONIOX_LIVE_PREVIEW_MIN_CUSTOM_WIDTH_PX as f64,
            SONIOX_LIVE_PREVIEW_MIN_CUSTOM_HEIGHT_PX as f64,
        )
        .max_inner_size(
            SONIOX_LIVE_PREVIEW_MAX_CUSTOM_WIDTH_PX as f64,
            SONIOX_LIVE_PREVIEW_MAX_CUSTOM_HEIGHT_PX as f64,
        )
        .maximizable(false)
        .minimizable(false)
        .closable(false)
        .accept_first_mouse(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .transparent(true)
        .focused(false)
        .visible(false);

        if let Some(data_dir) = crate::portable::data_dir() {
            builder = builder.data_directory(data_dir.join("webview"));
        }

        match builder.build()
        {
            Ok(_window) => {
                debug!("Live preview window created successfully (hidden)");
            }
            Err(e) => {
                debug!("Failed to create live preview window: {}", e);
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn create_soniox_live_preview_window(_app_handle: &AppHandle) {}

/// Creates the recording overlay panel and keeps it hidden by default (macOS)
#[cfg(target_os = "macos")]
pub fn create_recording_overlay(app_handle: &AppHandle) {
    let metrics =
        recording_overlay_window_metrics(app_handle, current_recording_overlay_layout());
    if let Some((x, y)) = calculate_overlay_position_for_window(app_handle, metrics) {
        // PanelBuilder creates a Tauri window then converts it to NSPanel.
        // The window remains registered, so get_webview_window() still works.
        match PanelBuilder::<_, RecordingOverlayPanel>::new(app_handle, "recording_overlay")
            .url(WebviewUrl::App("src/overlay/index.html".into()))
            .title("Recording")
            .position(tauri::Position::Logical(tauri::LogicalPosition { x, y }))
            .level(PanelLevel::Status)
            .size(tauri::Size::Logical(tauri::LogicalSize {
                width: metrics.window_width,
                height: metrics.window_height,
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

    // Cancel any pending transient message overlay auto-hide timer
    // by incrementing the generation counter
    TRANSIENT_OVERLAY_GENERATION.fetch_add(1, Ordering::SeqCst);

    // Check if overlay should be shown based on position setting
    let settings = settings::get_settings(app_handle);
    if settings.overlay_position == OverlayPosition::None {
        return;
    }

    set_recording_overlay_default_layout(app_handle);

    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
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

    set_recording_overlay_default_layout(app_handle);

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

    set_recording_overlay_default_layout(app_handle);

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

    set_recording_overlay_default_layout(app_handle);

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

    set_recording_overlay_default_layout(app_handle);

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
    let metrics =
        recording_overlay_window_metrics(app_handle, current_recording_overlay_layout());
    apply_recording_overlay_layout(app_handle, metrics);
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
        // When resuming a preview session (e.g., after Flush), the window is already
        // visible and positioned — skip repositioning to prevent jumping.
        let is_resuming = preview_output_mode_active && window.is_visible().unwrap_or(false);
        if !is_resuming {
            if let Some((x, y, width, height)) = resolve_soniox_live_preview_geometry(app_handle) {
                let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }));
                let _ =
                    window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
            }
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
    let should_show = app_settings.soniox_live_preview_enabled
        || crate::managers::preview_output_mode::is_active();
    if let Some(window) = app_handle.get_webview_window(SONIOX_LIVE_PREVIEW_WINDOW_LABEL) {
        if !should_show {
            let _ = window.hide();
        } else if let Some((x, y, width, height)) = resolve_soniox_live_preview_geometry(app_handle)
        {
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

        let app_settings = settings::get_settings(&app_handle);
        if !app_settings.soniox_live_preview_enabled {
            return Err(
                "Live Preview Window is disabled. Enable it in settings before opening demo preview."
                    .to_string(),
            );
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
                let _ =
                    window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
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

        return Err("Failed to open live preview window.".to_string());
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Live preview is available on Windows only.".to_string())
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
    let monitor = get_monitor_with_cursor(app_handle)?;
    let bounds = get_monitor_logical_auto_position_bounds(
        &monitor,
        settings::get_settings(app_handle).auto_position_allow_reserved_areas,
    );

    let x = bounds.x + (bounds.width - COMMAND_CONFIRM_WIDTH) / 2.0;
    let y = bounds.y + (bounds.height - COMMAND_CONFIRM_HEIGHT) / 2.0 - 50.0;

    Some((x, y))
}

/// Calculates bottom-center position for the floating voice activation button window.
fn calculate_voice_button_position(app_handle: &AppHandle) -> Option<(f64, f64)> {
    let monitor = get_monitor_with_cursor(app_handle)?;
    let bounds = get_monitor_logical_auto_position_bounds(
        &monitor,
        settings::get_settings(app_handle).auto_position_allow_reserved_areas,
    );

    let x = bounds.x + (bounds.width - VOICE_BUTTON_WIDTH) / 2.0;
    let y = bounds.y + bounds.height - VOICE_BUTTON_HEIGHT - 40.0;
    Some((x, y))
}

pub fn update_command_confirm_position(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window("command_confirm") {
        if let Some((x, y)) = calculate_command_confirm_position(app_handle) {
            let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        }
    }
}

#[cfg(target_os = "windows")]
pub fn update_voice_activation_button_position(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window("voice_activation_button") {
        if let Some((x, y)) = calculate_voice_button_position(app_handle) {
            let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn update_voice_activation_button_position(_app_handle: &AppHandle) {}

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
        let mut builder = WebviewWindowBuilder::new(
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
        .visible(false);

        if let Some(data_dir) = crate::portable::data_dir() {
            builder = builder.data_directory(data_dir.join("webview"));
        }

        match builder.build()
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
        let mut builder = WebviewWindowBuilder::new(
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
        .visible(false);

        if let Some(data_dir) = crate::portable::data_dir() {
            builder = builder.data_directory(data_dir.join("webview"));
        }

        match builder.build()
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
            let mut builder = WebviewWindowBuilder::new(
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
            .visible(false);

            if let Some(data_dir) = crate::portable::data_dir() {
                builder = builder.data_directory(data_dir.join("webview"));
            }

            match builder.build()
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

fn show_transient_message_overlay(
    app_handle: &AppHandle,
    overlay_state: &str,
    message: &str,
    auto_hide_ms: u64,
) {
    // Cancel pending error auto-hide timers so a new active overlay is not hidden.
    plus_overlay_state::invalidate_error_overlay_auto_hide();

    let settings = settings::get_settings(app_handle);
    if settings.overlay_position == OverlayPosition::None {
        return;
    }

    let show_overlay = {
        let session_state = app_handle.state::<crate::session_manager::ManagedSessionState>();
        let state_guard = session_state
            .lock()
            .expect("Failed to lock session state");
        matches!(*state_guard, crate::session_manager::SessionState::Idle)
    };

    if !show_overlay {
        return;
    }

    set_recording_overlay_default_layout(app_handle);

    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        let _ = overlay_window.show();

        #[cfg(target_os = "windows")]
        force_overlay_topmost(&overlay_window);

        let generation_at_start = TRANSIENT_OVERLAY_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

        let payload = TransientMessageOverlayPayload {
            state: overlay_state.to_string(),
            message: message.to_string(),
        };
        let _ = overlay_window.emit("show-message-overlay", payload);

        let window_clone = overlay_window.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(auto_hide_ms));

            if TRANSIENT_OVERLAY_GENERATION.load(Ordering::SeqCst) != generation_at_start {
                return;
            }

            let _ = window_clone.emit("hide-overlay", ());
            std::thread::sleep(std::time::Duration::from_millis(300));

            if TRANSIENT_OVERLAY_GENERATION.load(Ordering::SeqCst) != generation_at_start {
                return;
            }

            let _ = window_clone.hide();
        });
    }
}

// ============================================================================
// Profile Switch Overlay (Transcription Profiles)
// ============================================================================

/// Shows a brief overlay notification when switching transcription profiles.
/// Uses the existing recording overlay to display the profile name, then auto-hides.
pub fn show_profile_switch_overlay(app_handle: &AppHandle, profile_name: &str) {
    show_transient_message_overlay(app_handle, "profile_switch", profile_name, 1500);
}

/// Shows a brief overlay notification when the selected microphone changes.
/// Uses the existing recording overlay to display the new microphone name.
pub fn show_microphone_switch_overlay(app_handle: &AppHandle, microphone_name: &str) {
    show_transient_message_overlay(app_handle, "microphone_switch", microphone_name, 1500);
}

#[tauri::command]
#[specta::specta]
pub fn remember_recording_overlay_window_position(
    app_handle: AppHandle,
    x_px: i32,
    y_px: i32,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app_handle);
    let metrics =
        recording_overlay_window_metrics(&app_handle, current_recording_overlay_layout());
    settings.recording_overlay_use_manual_position = true;
    settings.recording_overlay_custom_x_px =
        (x_px as f64 + metrics.padding).round() as i32;
    settings.recording_overlay_custom_y_px =
        (y_px as f64 + metrics.padding).round() as i32;
    settings.recording_overlay_custom_x_px =
        settings.recording_overlay_custom_x_px.clamp(-10000, 10000);
    settings.recording_overlay_custom_y_px =
        settings.recording_overlay_custom_y_px.clamp(-10000, 10000);
    settings::write_settings(&app_handle, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn reset_recording_overlay_manual_position(app_handle: AppHandle) -> Result<(), String> {
    let mut settings = settings::get_settings(&app_handle);
    settings.recording_overlay_use_manual_position = false;
    settings.recording_overlay_custom_x_px = 0;
    settings.recording_overlay_custom_y_px = 0;
    settings::write_settings(&app_handle, settings);
    update_overlay_position(&app_handle);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_recording_overlay_appearance(
    app_handle: AppHandle,
) -> RecordingOverlayAppearancePayload {
    build_recording_overlay_appearance_payload(&app_handle)
}

#[derive(Clone, Copy)]
enum RecordingOverlayLayout {
    Default = 0,
    Error = 1,
}
