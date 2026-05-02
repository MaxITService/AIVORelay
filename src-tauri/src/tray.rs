use crate::managers::history::{HistoryEntry, HistoryManager};
use crate::managers::model::ModelManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::TranscriptionProvider;
use crate::tray_i18n::get_tray_translations;
use crate::url_security::{
    REMOTE_STT_GROQ_DEFAULT_MODEL, REMOTE_STT_OPENAI_DEFAULT_MODEL, REMOTE_STT_PRESET_CUSTOM,
    REMOTE_STT_PRESET_GROQ, REMOTE_STT_PRESET_OPENAI,
};
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
pub const TRAY_MODEL_MENU_PREFIX: &str = "tray_transcription_model::";
const TRAY_MICROPHONE_MISSING_ID: &str = "tray_microphone::missing";
const TRAY_MICROPHONE_HEADER_ID: &str = "tray_microphone_header";
const TRAY_MICROPHONE_HEADER_LABEL: &str = "Microphone";
const TRAY_MICROPHONE_DEFAULT_LABEL: &str = "Default";
const TRAY_MICROPHONE_UNAVAILABLE_PREFIX: &str = "Unavailable: ";
const TRAY_MODEL_SUBMENU_ID: &str = "model_submenu";
const TRAY_MODEL_LOCAL_HEADER_ID: &str = "tray_model_header::local";
const TRAY_MODEL_REMOTE_HEADER_ID: &str = "tray_model_header::remote_openai_compatible";
const TRAY_MODEL_SONIOX_HEADER_ID: &str = "tray_model_header::remote_soniox";
const TRAY_MODEL_DEEPGRAM_HEADER_ID: &str = "tray_model_header::remote_deepgram";
const TRAY_MODEL_LOCAL_LABEL: &str = "Local";
const TRAY_MODEL_REMOTE_LABEL: &str = "OpenAI-compatible";
const TRAY_MODEL_SONIOX_LABEL: &str = "Soniox";
const TRAY_MODEL_DEEPGRAM_LABEL: &str = "Deepgram";
const TRAY_MODEL_NO_LOCAL_MODELS_LABEL: &str = "No downloaded local models";
const TRAY_UNLOAD_LOCAL_MODEL_LABEL: &str = "Unload Local Model";
const TRAY_NO_LOCAL_MODEL_LOADED_LABEL: &str = "No Local Model Loaded";
const TRAY_SHORTCUT_GUIDE_LABEL: &str = "Shortcut Guide";
pub const TRAY_SHORTCUT_GUIDE_SHOW_IN_MAIN_ID: &str = "tray_shortcut_guide_show_in_main";
pub const TRAY_SHORTCUT_GUIDE_HIDE_FROM_MAIN_ID: &str = "tray_shortcut_guide_hide_from_main";
const TRAY_SHORTCUT_GUIDE_SHOW_IN_MAIN_LABEL: &str = "Show in Main Tray Menu";
const TRAY_SHORTCUT_GUIDE_HIDE_FROM_MAIN_LABEL: &str = "Hide shortcut guide from here";
const TRAY_SHORTCUT_GUIDE_ITEM_ICON: &str = "⌨️";
const TRAY_MODEL_CUSTOM_SUFFIX: &str = "Custom";
const TRAY_MODEL_PREFIX_LOCAL: &str = "local";
const TRAY_MODEL_PREFIX_REMOTE: &str = "remote_openai_compatible";
const TRAY_MODEL_PREFIX_SONIOX: &str = "remote_soniox";
const TRAY_MODEL_PREFIX_DEEPGRAM: &str = "remote_deepgram";

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayModelSelection {
    Local(String),
    RemoteOpenAiCompatible {
        provider_preset: String,
        model_id: String,
    },
    RemoteSoniox(String),
    RemoteDeepgram(String),
}

pub fn parse_model_menu_selection(id: &str) -> Option<TrayModelSelection> {
    let selection = id.strip_prefix(TRAY_MODEL_MENU_PREFIX)?;
    let (provider, value) = selection.split_once("::")?;
    if value.trim().is_empty() {
        return None;
    }

    match provider {
        TRAY_MODEL_PREFIX_LOCAL => Some(TrayModelSelection::Local(value.to_string())),
        TRAY_MODEL_PREFIX_REMOTE => {
            let (provider_preset, model_id) = value.split_once("::")?;
            if provider_preset.trim().is_empty() || model_id.trim().is_empty() {
                return None;
            }
            Some(TrayModelSelection::RemoteOpenAiCompatible {
                provider_preset: provider_preset.to_string(),
                model_id: model_id.to_string(),
            })
        }
        TRAY_MODEL_PREFIX_SONIOX => Some(TrayModelSelection::RemoteSoniox(value.to_string())),
        TRAY_MODEL_PREFIX_DEEPGRAM => Some(TrayModelSelection::RemoteDeepgram(value.to_string())),
        _ => None,
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
    let unload_model_label = if model_loaded {
        TRAY_UNLOAD_LOCAL_MODEL_LABEL
    } else if settings.transcription_provider != TranscriptionProvider::Local {
        TRAY_NO_LOCAL_MODEL_LOADED_LABEL
    } else if strings.unload_model.is_empty() {
        "Unload Model"
    } else {
        &strings.unload_model
    };
    let unload_model_i = MenuItem::with_id(
        app,
        "unload_model",
        unload_model_label,
        model_loaded,
        None::<&str>,
    )?;
    let model_menu_label = build_model_menu_label(app, &settings, &strings.model);
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
        let model_submenu = build_model_submenu(app, &model_menu_label, &settings)?;
        menu.append(&separator()?)?;
        menu.append(&model_submenu)?;
        menu.append(&unload_model_i)?;
        if settings.show_tray_shortcut_guide {
            if settings.show_tray_shortcut_guide_in_main_menu {
                append_shortcut_guide_main_menu_items(&menu, app, &settings)?;
            } else if let Some(guide_submenu) = build_shortcut_guide_submenu(app, &settings)? {
                menu.append(&separator()?)?;
                menu.append(&guide_submenu)?;
            }
        }
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

fn build_model_menu_label(
    app: &AppHandle,
    settings: &settings::AppSettings,
    fallback_label: &str,
) -> String {
    let fallback_label = if fallback_label.is_empty() {
        "Model"
    } else {
        fallback_label
    };

    match settings.transcription_provider {
        TranscriptionProvider::Local => {
            let model_manager = app.state::<Arc<ModelManager>>();
            let selected_name = model_manager
                .get_available_models()
                .into_iter()
                .find(|model| model.id == settings.selected_model.as_str())
                .map(|model| model.name)
                .unwrap_or_else(|| fallback_label.to_string());
            format!("{TRAY_MODEL_LOCAL_LABEL}: {selected_name}")
        }
        TranscriptionProvider::RemoteOpenAiCompatible => {
            let provider_label = match settings.remote_stt.provider_preset.as_str() {
                REMOTE_STT_PRESET_GROQ => "Groq",
                REMOTE_STT_PRESET_OPENAI => "OpenAI",
                REMOTE_STT_PRESET_CUSTOM => TRAY_MODEL_CUSTOM_SUFFIX,
                _ => TRAY_MODEL_REMOTE_LABEL,
            };
            format!("{provider_label}: {}", settings.remote_stt.model_id)
        }
        TranscriptionProvider::RemoteSoniox => {
            format!("{TRAY_MODEL_SONIOX_LABEL}: {}", settings.soniox_model)
        }
        TranscriptionProvider::RemoteDeepgram => {
            format!("{TRAY_MODEL_DEEPGRAM_LABEL}: {}", settings.deepgram_model)
        }
    }
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
    settings: &settings::AppSettings,
) -> Result<Submenu<tauri::Wry>, Box<dyn std::error::Error>> {
    let submenu = Submenu::with_id(app, TRAY_MODEL_SUBMENU_ID, label, true)?;
    append_local_model_items(&submenu, app, settings)?;
    submenu.append(&PredefinedMenuItem::separator(app)?)?;
    append_remote_openai_model_items(&submenu, app, settings)?;
    submenu.append(&PredefinedMenuItem::separator(app)?)?;
    append_soniox_model_items(&submenu, app, settings)?;
    submenu.append(&PredefinedMenuItem::separator(app)?)?;
    append_deepgram_model_items(&submenu, app, settings)?;

    Ok(submenu)
}

fn append_submenu_header(
    submenu: &Submenu<tauri::Wry>,
    app: &AppHandle,
    id: &str,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let header = MenuItem::with_id(app, id, label, false, None::<&str>)?;
    submenu.append(&header)?;
    Ok(())
}

fn append_local_model_items(
    submenu: &Submenu<tauri::Wry>,
    app: &AppHandle,
    settings: &settings::AppSettings,
) -> Result<(), Box<dyn std::error::Error>> {
    append_submenu_header(
        submenu,
        app,
        TRAY_MODEL_LOCAL_HEADER_ID,
        TRAY_MODEL_LOCAL_LABEL,
    )?;

    let model_manager = app.state::<Arc<ModelManager>>();
    let mut downloaded_models: Vec<_> = model_manager
        .get_available_models()
        .into_iter()
        .filter(|model| model.is_downloaded)
        .collect();

    if downloaded_models.is_empty() {
        let item = MenuItem::with_id(
            app,
            "tray_model_no_local_models",
            TRAY_MODEL_NO_LOCAL_MODELS_LABEL,
            false,
            None::<&str>,
        )?;
        submenu.append(&item)?;
        return Ok(());
    }

    downloaded_models.sort_by(|left, right| left.name.cmp(&right.name));

    for model in downloaded_models {
        let item = CheckMenuItem::with_id(
            app,
            model_menu_id(TRAY_MODEL_PREFIX_LOCAL, &model.id),
            &model.name,
            true,
            settings.transcription_provider == TranscriptionProvider::Local
                && model.id == settings.selected_model,
            None::<&str>,
        )?;
        submenu.append(&item)?;
    }

    Ok(())
}

fn append_remote_openai_model_items(
    submenu: &Submenu<tauri::Wry>,
    app: &AppHandle,
    settings: &settings::AppSettings,
) -> Result<(), Box<dyn std::error::Error>> {
    append_submenu_header(
        submenu,
        app,
        TRAY_MODEL_REMOTE_HEADER_ID,
        TRAY_MODEL_REMOTE_LABEL,
    )?;

    let mut models = vec![
        (
            REMOTE_STT_PRESET_GROQ.to_string(),
            REMOTE_STT_GROQ_DEFAULT_MODEL.to_string(),
            "Groq: whisper-large-v3-turbo".to_string(),
        ),
        (
            REMOTE_STT_PRESET_GROQ.to_string(),
            "whisper-large-v3".to_string(),
            "Groq: whisper-large-v3".to_string(),
        ),
        (
            REMOTE_STT_PRESET_OPENAI.to_string(),
            REMOTE_STT_OPENAI_DEFAULT_MODEL.to_string(),
            "OpenAI: whisper-1".to_string(),
        ),
    ];

    let current_model = settings.remote_stt.model_id.trim();
    let current_preset = match settings.remote_stt.provider_preset.trim() {
        "" => REMOTE_STT_PRESET_CUSTOM,
        preset => preset,
    };
    if !current_model.is_empty()
        && !models
            .iter()
            .any(|(preset, model_id, _)| preset == current_preset && model_id == current_model)
    {
        models.push((
            current_preset.to_string(),
            current_model.to_string(),
            format!("{TRAY_MODEL_CUSTOM_SUFFIX}: {current_model}"),
        ));
    }

    for (provider_preset, model_id, label) in models {
        let item = CheckMenuItem::with_id(
            app,
            remote_openai_model_menu_id(&provider_preset, &model_id),
            &label,
            true,
            settings.transcription_provider == TranscriptionProvider::RemoteOpenAiCompatible
                && provider_preset == settings.remote_stt.provider_preset
                && model_id == settings.remote_stt.model_id,
            None::<&str>,
        )?;
        submenu.append(&item)?;
    }

    Ok(())
}

fn append_soniox_model_items(
    submenu: &Submenu<tauri::Wry>,
    app: &AppHandle,
    settings: &settings::AppSettings,
) -> Result<(), Box<dyn std::error::Error>> {
    append_submenu_header(
        submenu,
        app,
        TRAY_MODEL_SONIOX_HEADER_ID,
        TRAY_MODEL_SONIOX_LABEL,
    )?;

    let mut models = vec![
        (
            settings::SONIOX_DEFAULT_MODEL.to_string(),
            settings::SONIOX_DEFAULT_MODEL.to_string(),
        ),
        ("stt-async-v4".to_string(), "stt-async-v4".to_string()),
    ];
    let current_model = settings.soniox_model.trim();
    if !current_model.is_empty() && !models.iter().any(|(model_id, _)| model_id == current_model) {
        models.push((current_model.to_string(), current_model.to_string()));
    }

    append_provider_model_items(
        submenu,
        app,
        TRAY_MODEL_PREFIX_SONIOX,
        TranscriptionProvider::RemoteSoniox,
        &settings.transcription_provider,
        &settings.soniox_model,
        models,
    )
}

fn append_deepgram_model_items(
    submenu: &Submenu<tauri::Wry>,
    app: &AppHandle,
    settings: &settings::AppSettings,
) -> Result<(), Box<dyn std::error::Error>> {
    append_submenu_header(
        submenu,
        app,
        TRAY_MODEL_DEEPGRAM_HEADER_ID,
        TRAY_MODEL_DEEPGRAM_LABEL,
    )?;

    let mut models = vec![
        (
            settings::DEEPGRAM_DEFAULT_MODEL.to_string(),
            settings::DEEPGRAM_DEFAULT_MODEL.to_string(),
        ),
        ("nova-3-general".to_string(), "nova-3-general".to_string()),
        ("nova-3-medical".to_string(), "nova-3-medical".to_string()),
    ];
    let current_model = settings.deepgram_model.trim();
    if !current_model.is_empty() && !models.iter().any(|(model_id, _)| model_id == current_model) {
        models.push((current_model.to_string(), current_model.to_string()));
    }

    append_provider_model_items(
        submenu,
        app,
        TRAY_MODEL_PREFIX_DEEPGRAM,
        TranscriptionProvider::RemoteDeepgram,
        &settings.transcription_provider,
        &settings.deepgram_model,
        models,
    )
}

fn append_provider_model_items(
    submenu: &Submenu<tauri::Wry>,
    app: &AppHandle,
    provider_prefix: &str,
    provider: TranscriptionProvider,
    current_provider: &TranscriptionProvider,
    current_model_id: &str,
    models: Vec<(String, String)>,
) -> Result<(), Box<dyn std::error::Error>> {
    for (model_id, label) in models {
        let item = CheckMenuItem::with_id(
            app,
            model_menu_id(provider_prefix, &model_id),
            &label,
            true,
            *current_provider == provider && model_id == current_model_id,
            None::<&str>,
        )?;
        submenu.append(&item)?;
    }

    Ok(())
}

fn model_menu_id(provider_prefix: &str, model_id: &str) -> String {
    format!("{TRAY_MODEL_MENU_PREFIX}{provider_prefix}::{model_id}")
}

fn remote_openai_model_menu_id(provider_preset: &str, model_id: &str) -> String {
    format!("{TRAY_MODEL_MENU_PREFIX}{TRAY_MODEL_PREFIX_REMOTE}::{provider_preset}::{model_id}")
}

fn build_shortcut_guide_submenu(
    app: &AppHandle,
    settings: &settings::AppSettings,
) -> Result<Option<Submenu<tauri::Wry>>, Box<dyn std::error::Error>> {
    let sections = crate::hotkey_guide::build_hotkey_guide_sections(settings);
    if sections.is_empty() {
        return Ok(None);
    }

    let submenu = Submenu::with_id(app, "tray_shortcut_guide", TRAY_SHORTCUT_GUIDE_LABEL, true)?;

    for section in sections {
        for binding in section.bindings {
            let item = MenuItem::with_id(
                app,
                format!("tray_shortcut_guide_item::{}", binding.id),
                shortcut_guide_item_label(&binding.name, &binding.current_binding),
                false,
                None::<&str>,
            )?;
            submenu.append(&item)?;
        }
    }

    let show_in_main = MenuItem::with_id(
        app,
        TRAY_SHORTCUT_GUIDE_SHOW_IN_MAIN_ID,
        TRAY_SHORTCUT_GUIDE_SHOW_IN_MAIN_LABEL,
        true,
        None::<&str>,
    )?;
    submenu.append(&show_in_main)?;

    Ok(Some(submenu))
}

fn append_shortcut_guide_main_menu_items(
    menu: &Menu<tauri::Wry>,
    app: &AppHandle,
    settings: &settings::AppSettings,
) -> Result<(), Box<dyn std::error::Error>> {
    let sections = crate::hotkey_guide::build_hotkey_guide_sections(settings);
    if sections.is_empty() {
        return Ok(());
    }

    let title = MenuItem::with_id(
        app,
        "tray_shortcut_guide_main_header",
        TRAY_SHORTCUT_GUIDE_LABEL,
        false,
        None::<&str>,
    )?;
    menu.append(&title)?;

    for section in sections {
        for binding in section.bindings {
            let item = MenuItem::with_id(
                app,
                format!("tray_shortcut_guide_main_item::{}", binding.id),
                shortcut_guide_item_label(&binding.name, &binding.current_binding),
                false,
                None::<&str>,
            )?;
            menu.append(&item)?;
        }
    }

    let hide_from_main = MenuItem::with_id(
        app,
        TRAY_SHORTCUT_GUIDE_HIDE_FROM_MAIN_ID,
        TRAY_SHORTCUT_GUIDE_HIDE_FROM_MAIN_LABEL,
        true,
        None::<&str>,
    )?;
    menu.append(&hide_from_main)?;

    Ok(())
}

fn shortcut_guide_item_label(name: &str, binding: &str) -> String {
    format!(
        "{TRAY_SHORTCUT_GUIDE_ITEM_ICON} {name} - {}",
        format_shortcut_for_tray(binding)
    )
}

fn format_shortcut_for_tray(binding: &str) -> String {
    binding
        .split('+')
        .map(|part| {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                return String::new();
            }
            let mut chars = trimmed.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" + ")
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
    use super::{
        get_icon_path, last_transcript_text, parse_microphone_menu_selection,
        parse_model_menu_selection, tray_tooltip, AppTheme, TrayIconState, TrayModelSelection,
        TRAY_MICROPHONE_DEFAULT_ID, TRAY_MICROPHONE_MENU_PREFIX, TRAY_MICROPHONE_MISSING_ID,
        TRAY_MODEL_MENU_PREFIX,
    };
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

    #[test]
    fn get_icon_path_returns_expected_resources_for_dark_theme() {
        assert_eq!(
            get_icon_path(AppTheme::Dark, TrayIconState::Idle),
            "resources/aivo_tray.png"
        );
        assert_eq!(
            get_icon_path(AppTheme::Dark, TrayIconState::Recording),
            "resources/tray_recording.png"
        );
        assert_eq!(
            get_icon_path(AppTheme::Dark, TrayIconState::Transcribing),
            "resources/tray_transcribing.png"
        );
    }

    #[test]
    fn get_icon_path_returns_expected_resources_for_light_and_colored_themes() {
        assert_eq!(
            get_icon_path(AppTheme::Light, TrayIconState::Recording),
            "resources/tray_recording_dark.png"
        );
        assert_eq!(
            get_icon_path(AppTheme::Light, TrayIconState::Transcribing),
            "resources/tray_transcribing_dark.png"
        );
        assert_eq!(
            get_icon_path(AppTheme::Colored, TrayIconState::Recording),
            "resources/recording.png"
        );
        assert_eq!(
            get_icon_path(AppTheme::Colored, TrayIconState::Transcribing),
            "resources/transcribing.png"
        );
    }

    #[test]
    fn tray_tooltip_uses_app_version_label() {
        let tooltip = tray_tooltip();

        assert!(tooltip.contains(env!("CARGO_PKG_VERSION")));
        #[cfg(debug_assertions)]
        assert!(tooltip.contains("(Dev)"));
    }

    #[test]
    fn parse_microphone_menu_selection_handles_special_ids() {
        assert_eq!(
            parse_microphone_menu_selection(TRAY_MICROPHONE_DEFAULT_ID),
            Some(None)
        );
        assert_eq!(
            parse_microphone_menu_selection(TRAY_MICROPHONE_MISSING_ID),
            None
        );
    }

    #[test]
    fn parse_microphone_menu_selection_extracts_device_index_suffix() {
        let id = format!("{TRAY_MICROPHONE_MENU_PREFIX}7");
        assert_eq!(
            parse_microphone_menu_selection(&id),
            Some(Some("7".to_string()))
        );
        assert_eq!(parse_microphone_menu_selection("some-other-id"), None);
    }

    #[test]
    fn parse_model_menu_selection_extracts_provider_and_model() {
        assert_eq!(
            parse_model_menu_selection(&format!("{TRAY_MODEL_MENU_PREFIX}local::ggml-small")),
            Some(TrayModelSelection::Local("ggml-small".to_string()))
        );
        assert_eq!(
            parse_model_menu_selection(&format!(
                "{TRAY_MODEL_MENU_PREFIX}remote_openai_compatible::openai::whisper-1"
            )),
            Some(TrayModelSelection::RemoteOpenAiCompatible {
                provider_preset: "openai".to_string(),
                model_id: "whisper-1".to_string(),
            })
        );
        assert_eq!(
            parse_model_menu_selection(&format!(
                "{TRAY_MODEL_MENU_PREFIX}remote_soniox::stt-rt-v4"
            )),
            Some(TrayModelSelection::RemoteSoniox("stt-rt-v4".to_string()))
        );
        assert_eq!(parse_model_menu_selection("some-other-id"), None);
    }
}
