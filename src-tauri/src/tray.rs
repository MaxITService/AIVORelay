use crate::managers::history::{HistoryEntry, HistoryManager};
use crate::managers::model::ModelManager;
use crate::managers::transcription::TranscriptionManager;
use crate::tray_i18n::get_tray_translations;
use crate::{commands::audio, settings};
use log::{error, info, warn};
use std::sync::{Arc, Mutex};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIcon;
use tauri::{AppHandle, Manager, Theme};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[derive(Clone, Debug, PartialEq)]
pub enum TrayIconState {
    Idle,
    Recording,
    Transcribing,
}

pub struct ManagedTrayState(pub Mutex<TrayIconState>);

impl Default for ManagedTrayState {
    fn default() -> Self {
        Self(Mutex::new(TrayIconState::Idle))
    }
}

pub const TRAY_MICROPHONE_MENU_PREFIX: &str = "tray_microphone::";
pub const TRAY_MICROPHONE_DEFAULT_ID: &str = "tray_microphone::default";
pub const TRAY_MODEL_MENU_PREFIX: &str = "model_select:";
const TRAY_MICROPHONE_MISSING_ID: &str = "tray_microphone::missing";
const TRAY_MICROPHONE_HEADER_ID: &str = "tray_microphone_header";
const TRAY_MICROPHONE_HEADER_LABEL: &str = "Microphone";
const TRAY_MICROPHONE_DEFAULT_LABEL: &str = "Default";
const TRAY_MICROPHONE_UNAVAILABLE_PREFIX: &str = "Unavailable: ";

#[derive(Clone, Debug, PartialEq)]
pub enum AppTheme {
    Dark,
    Light,
    Colored, // Pink/colored theme for Linux
}

/// Gets the current app theme, with Linux defaulting to Colored theme
pub fn get_current_theme(app: &AppHandle) -> AppTheme {
    if cfg!(target_os = "linux") {
        // On Linux, always use the colored theme
        AppTheme::Colored
    } else {
        // On other platforms, map system theme to our app theme
        if let Some(main_window) = app.get_webview_window("main") {
            match main_window.theme().unwrap_or(Theme::Dark) {
                Theme::Light => AppTheme::Light,
                Theme::Dark => AppTheme::Dark,
                _ => AppTheme::Dark, // Default fallback
            }
        } else {
            AppTheme::Dark
        }
    }
}

/// Gets the appropriate icon path for the given theme and state
pub fn get_icon_path(theme: AppTheme, state: TrayIconState) -> &'static str {
    match (theme, state) {
        // Dark theme uses light icons
        (AppTheme::Dark, TrayIconState::Idle) => "resources/aivo_tray.png",
        (AppTheme::Dark, TrayIconState::Recording) => "resources/tray_recording.png",
        (AppTheme::Dark, TrayIconState::Transcribing) => "resources/tray_transcribing.png",
        // Light theme uses dark icons
        (AppTheme::Light, TrayIconState::Idle) => "resources/aivo_tray.png",
        (AppTheme::Light, TrayIconState::Recording) => "resources/tray_recording_dark.png",
        (AppTheme::Light, TrayIconState::Transcribing) => "resources/tray_transcribing_dark.png",
        // Colored theme uses pink icons (for Linux)
        (AppTheme::Colored, TrayIconState::Idle) => "resources/aivo_tray.png",
        (AppTheme::Colored, TrayIconState::Recording) => "resources/recording.png",
        (AppTheme::Colored, TrayIconState::Transcribing) => "resources/transcribing.png",
    }
}

pub fn change_tray_icon(app: &AppHandle, icon: TrayIconState) {
    let tray = app.state::<TrayIcon>();
    let theme = get_current_theme(app);

    let icon_path = get_icon_path(theme, icon.clone());

    match app
        .path()
        .resolve(icon_path, tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())
        .and_then(|p| Image::from_path(p).map_err(|e| e.to_string()))
    {
        Ok(image) => {
            let _ = tray.set_icon(Some(image));
        }
        Err(e) => {
            warn!("Failed to update tray icon '{}': {}", icon_path, e);
        }
    }

    // Update menu based on state
    update_tray_menu(app, &icon, None);
}

pub fn tray_tooltip() -> String {
    version_label()
}

fn version_label() -> String {
    if cfg!(debug_assertions) {
        format!("AivoRelay v{} (Dev)", env!("CARGO_PKG_VERSION"))
    } else {
        format!("AivoRelay v{}", env!("CARGO_PKG_VERSION"))
    }
}

pub fn update_tray_menu(app: &AppHandle, state: &TrayIconState, locale: Option<&str>) {
    remember_tray_state(app, state);
    if let Err(e) = try_update_tray_menu(app, state, locale) {
        warn!("Failed to update tray menu: {}", e);
    }
}

pub fn refresh_tray_menu(app: &AppHandle, locale: Option<&str>) {
    let state = current_tray_state(app);
    update_tray_menu(app, &state, locale);
}

pub fn parse_microphone_menu_selection(id: &str) -> Option<Option<String>> {
    if id == TRAY_MICROPHONE_DEFAULT_ID {
        Some(None)
    } else if id == TRAY_MICROPHONE_MISSING_ID {
        None
    } else {
        id.strip_prefix(TRAY_MICROPHONE_MENU_PREFIX)
            .map(|index| Some(index.to_string()))
    }
}

fn try_update_tray_menu(
    app: &AppHandle,
    state: &TrayIconState,
    locale: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let settings = settings::get_settings(app);

    let locale = locale.unwrap_or(&settings.app_language);
    let strings = get_tray_translations(Some(locale.to_string()));

    // Platform-specific accelerators
    #[cfg(target_os = "macos")]
    let (settings_accelerator, quit_accelerator) = (Some("Cmd+,"), Some("Cmd+Q"));
    #[cfg(not(target_os = "macos"))]
    let (settings_accelerator, quit_accelerator) = (Some("Ctrl+,"), Some("Ctrl+Q"));

    // Create common menu items
    let version_label = version_label();
    let version_i = MenuItem::with_id(app, "version", &version_label, false, None::<&str>)?;
    let settings_i = MenuItem::with_id(
        app,
        "settings",
        &strings.settings,
        true,
        settings_accelerator,
    )?;
    let check_updates_i = MenuItem::with_id(
        app,
        "check_updates",
        &strings.check_updates,
        settings.update_checks_enabled,
        None::<&str>,
    )?;
    let copy_last_transcript_i = MenuItem::with_id(
        app,
        "copy_last_transcript",
        &strings.copy_last_transcript,
        true,
        None::<&str>,
    )?;
    let model_loaded = app.state::<Arc<TranscriptionManager>>().is_model_loaded();
    let unload_model_i = MenuItem::with_id(
        app,
        "unload_model",
        &strings.unload_model,
        model_loaded,
        None::<&str>,
    )?;
    let model_menu_label = {
        let fallback_label = if strings.model.is_empty() {
            "Model"
        } else {
            &strings.model
        };
        let model_manager = app.state::<Arc<ModelManager>>();
        model_manager
            .get_available_models()
            .into_iter()
            .find(|model| model.id == settings.selected_model.as_str())
            .map(|model| model.name)
            .unwrap_or_else(|| fallback_label.to_string())
    };
    let quit_i = MenuItem::with_id(app, "quit", &strings.quit, true, quit_accelerator)?;
    let separator = || PredefinedMenuItem::separator(app);

    let menu = Menu::new(app)?;

    match state {
        TrayIconState::Recording | TrayIconState::Transcribing => {
            let cancel_i = MenuItem::with_id(app, "cancel", &strings.cancel, true, None::<&str>)?;
            menu.append(&version_i)?;
            menu.append(&separator()?)?;
            menu.append(&cancel_i)?;
        }
        TrayIconState::Idle => {
            menu.append(&version_i)?;
        }
    }

    menu.append(&separator()?)?;
    append_microphone_items(&menu, app, settings.selected_microphone.as_deref())?;
    menu.append(&separator()?)?;
    menu.append(&copy_last_transcript_i)?;

    if state == &TrayIconState::Idle {
        if let Some(model_submenu) =
            build_model_submenu(app, &model_menu_label, &settings.selected_model)?
        {
            menu.append(&separator()?)?;
            menu.append(&model_submenu)?;
        }
        menu.append(&unload_model_i)?;
    }

    menu.append(&separator()?)?;
    menu.append(&settings_i)?;
    menu.append(&check_updates_i)?;
    menu.append(&separator()?)?;
    menu.append(&quit_i)?;

    let Some(tray) = app.try_state::<TrayIcon>() else {
        return Ok(());
    };
    let _ = tray.set_menu(Some(menu));
    let _ = tray.set_icon_as_template(true);
    let _ = tray.set_tooltip(Some(version_label));
    Ok(())
}

fn remember_tray_state(app: &AppHandle, state: &TrayIconState) {
    let Some(current_state) = app.try_state::<ManagedTrayState>() else {
        return;
    };

    let lock_result = current_state.0.lock();
    match lock_result {
        Ok(mut current_state) => {
            *current_state = state.clone();
        }
        Err(err) => {
            warn!("Failed to lock tray state while updating menu: {}", err);
        }
    }
}

fn current_tray_state(app: &AppHandle) -> TrayIconState {
    let Some(current_state) = app.try_state::<ManagedTrayState>() else {
        return TrayIconState::Idle;
    };

    let state = match current_state.0.lock() {
        Ok(current_state) => current_state.clone(),
        Err(err) => {
            warn!("Failed to lock tray state while refreshing menu: {}", err);
            TrayIconState::Idle
        }
    };

    state
}

fn append_microphone_items(
    menu: &Menu<tauri::Wry>,
    app: &AppHandle,
    selected_microphone: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let header_item = MenuItem::with_id(
        app,
        TRAY_MICROPHONE_HEADER_ID,
        TRAY_MICROPHONE_HEADER_LABEL,
        false,
        None::<&str>,
    )?;
    menu.append(&header_item)?;

    let available_microphones = match audio::get_available_microphones() {
        Ok(devices) => devices,
        Err(err) => {
            warn!("Failed to list microphones for tray menu: {}", err);
            vec![audio::AudioDevice {
                index: "default".to_string(),
                name: TRAY_MICROPHONE_DEFAULT_LABEL.to_string(),
                is_default: true,
            }]
        }
    };

    let missing_selected_microphone = selected_microphone.filter(|selected_name| {
        !available_microphones
            .iter()
            .any(|device| !device.is_default && device.name == *selected_name)
    });
    let default_item = CheckMenuItem::with_id(
        app,
        TRAY_MICROPHONE_DEFAULT_ID,
        TRAY_MICROPHONE_DEFAULT_LABEL,
        true,
        selected_microphone.is_none(),
        None::<&str>,
    )?;
    menu.append(&default_item)?;

    if let Some(selected_name) = missing_selected_microphone {
        let unavailable_item = CheckMenuItem::with_id(
            app,
            TRAY_MICROPHONE_MISSING_ID,
            format!("{TRAY_MICROPHONE_UNAVAILABLE_PREFIX}{selected_name}"),
            false,
            true,
            None::<&str>,
        )?;
        menu.append(&unavailable_item)?;
    }

    for device in available_microphones
        .into_iter()
        .filter(|device| !device.is_default)
    {
        let item = CheckMenuItem::with_id(
            app,
            format!("{}{}", TRAY_MICROPHONE_MENU_PREFIX, device.index),
            &device.name,
            true,
            selected_microphone == Some(device.name.as_str()),
            None::<&str>,
        )?;
        menu.append(&item)?;
    }

    Ok(())
}

fn build_model_submenu(
    app: &AppHandle,
    label: &str,
    current_model_id: &str,
) -> Result<Option<Submenu<tauri::Wry>>, Box<dyn std::error::Error>> {
    let model_manager = app.state::<Arc<ModelManager>>();
    let mut downloaded_models: Vec<_> = model_manager
        .get_available_models()
        .into_iter()
        .filter(|model| model.is_downloaded)
        .collect();

    if downloaded_models.is_empty() {
        return Ok(None);
    }

    downloaded_models.sort_by(|left, right| left.name.cmp(&right.name));

    let submenu = Submenu::with_id(app, "model_submenu", label, true)?;

    for model in downloaded_models {
        let item = CheckMenuItem::with_id(
            app,
            format!("{TRAY_MODEL_MENU_PREFIX}{}", model.id),
            &model.name,
            true,
            model.id == current_model_id,
            None::<&str>,
        )?;
        submenu.append(&item)?;
    }

    Ok(Some(submenu))
}

pub fn set_tray_visibility(app: &AppHandle, visible: bool) {
    let Some(tray) = app.try_state::<TrayIcon>() else {
        warn!("Tray icon state unavailable while setting visibility.");
        return;
    };

    if let Err(err) = tray.set_visible(visible) {
        error!("Failed to set tray visibility: {}", err);
    } else {
        info!("Tray visibility set to {}", visible);
    }
}

fn last_transcript_text(entry: &HistoryEntry) -> &str {
    entry
        .post_processed_text
        .as_deref()
        .unwrap_or(&entry.transcription_text)
}

pub fn copy_last_transcript(app: &AppHandle) {
    let history_manager = app.state::<Arc<HistoryManager>>();
    let entry = match history_manager.get_latest_completed_entry() {
        Ok(Some(entry)) => entry,
        Ok(None) => {
            warn!("No completed transcription history entries available for tray copy.");
            return;
        }
        Err(err) => {
            error!(
                "Failed to fetch last completed transcription entry: {}",
                err
            );
            return;
        }
    };

    let text = last_transcript_text(&entry);
    if text.trim().is_empty() {
        warn!("Last completed transcription is empty; skipping tray copy.");
        return;
    }

    if let Err(err) = app.clipboard().write_text(text) {
        error!("Failed to copy last transcript to clipboard: {}", err);
        return;
    }

    info!("Copied last transcript to clipboard via tray.");
}

#[cfg(test)]
mod tests {
    use super::last_transcript_text;
    use crate::managers::history::HistoryEntry;

    fn build_entry(transcription: &str, post_processed: Option<&str>) -> HistoryEntry {
        HistoryEntry {
            id: 1,
            file_name: "handy-1.wav".to_string(),
            timestamp: 0,
            saved: false,
            title: "Recording".to_string(),
            transcription_text: transcription.to_string(),
            post_processed_text: post_processed.map(|text| text.to_string()),
            post_process_prompt: None,
            post_process_requested: false,
            action_type: "transcribe".to_string(),
            original_selection: None,
            ai_response: None,
        }
    }

    #[test]
    fn uses_post_processed_text_when_available() {
        let entry = build_entry("raw", Some("processed"));
        assert_eq!(last_transcript_text(&entry), "processed");
    }

    #[test]
    fn falls_back_to_raw_transcription() {
        let entry = build_entry("raw", None);
        assert_eq!(last_transcript_text(&entry), "raw");
    }
}
