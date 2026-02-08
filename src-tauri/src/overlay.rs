use crate::input;
use crate::plus_overlay_state;
use crate::settings;
use crate::settings::OverlayPosition;
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
                // don't subtract the overlay height it puts it too far up
                work_area_y + work_area_height - OVERLAY_BOTTOM_OFFSET
            }
        };

        return Some((x, y));
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
        let _ = overlay_window.emit("show-overlay", "recording");
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
        let _ = overlay_window.emit("show-overlay", "transcribing");
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
        let _ = overlay_window.emit("show-overlay", "sending");
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
        let _ = overlay_window.emit("show-overlay", "thinking");
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
        // Hide the window after a short delay to allow animation to complete
        let window_clone = overlay_window.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let _ = window_clone.hide();
        });
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
