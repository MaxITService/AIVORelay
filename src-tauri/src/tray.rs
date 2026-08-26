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
use log::{debug, error, info, trace, warn};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIcon;
use tauri::{AppHandle, Manager, Theme};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayIconState {
    Idle,
    Recording,
    Transcribing,
}

impl TrayIconState {
    fn is_busy(self) -> bool {
        self != TrayIconState::Idle
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TrayModelItem {
    id: String,
    name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TrayMicrophoneItem {
    index: String,
    name: String,
    is_default: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TrayShortcutItem {
    id: String,
    label: String,
}

/// Everything that can change the visible tray menu.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MenuInputs {
    busy: bool,
    locale: String,
    update_checks_enabled: bool,
    transcription_provider: TranscriptionProvider,
    selected_model: String,
    selected_local_model_name: Option<String>,
    selected_microphone: Option<String>,
    remote_provider_preset: String,
    remote_model_id: String,
    soniox_model: String,
    deepgram_model: String,
    show_shortcut_guide: bool,
    show_shortcut_guide_in_main_menu: bool,
    model_loaded: bool,
    downloaded_local_models: Vec<TrayModelItem>,
    microphones: Vec<TrayMicrophoneItem>,
    shortcut_items: Vec<TrayShortcutItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TrayDesired {
    icon_path: &'static str,
    menu: MenuInputs,
}

struct TrayInner {
    icon_state: TrayIconState,
    desired: Option<TrayDesired>,
    applied_icon: Option<&'static str>,
    applied_menu: Option<MenuInputs>,
    pending: bool,
    icons: HashMap<&'static str, Image<'static>>,
    next_seq: u64,
    desired_seq: u64,
}

/// Owns the desired and applied tray snapshots. Native tray mutations are
/// performed only by the main-thread applier.
pub struct TrayState(Mutex<TrayInner>);

impl TrayState {
    pub fn new() -> Self {
        Self(Mutex::new(TrayInner {
            icon_state: TrayIconState::Idle,
            desired: None,
            applied_icon: None,
            applied_menu: None,
            pending: false,
            icons: HashMap::new(),
            next_seq: 0,
            desired_seq: 0,
        }))
    }

    fn lock(&self) -> MutexGuard<'_, TrayInner> {
        self.0.lock().unwrap_or_else(|poisoned| {
            warn!("Tray state mutex was poisoned; recovering");
            poisoned.into_inner()
        })
    }
}

impl Default for TrayState {
    fn default() -> Self {
        Self::new()
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
const TRAY_SHORTCUT_GUIDE_LABEL: &str = "Here are the keys you set in program:";
pub const TRAY_SHORTCUT_GUIDE_SHOW_IN_MAIN_ID: &str = "tray_shortcut_guide_show_in_main";
pub const TRAY_SHORTCUT_GUIDE_HIDE_FROM_MAIN_ID: &str = "tray_shortcut_guide_hide_from_main";
const TRAY_SHORTCUT_GUIDE_SHOW_IN_MAIN_LABEL: &str = "Show in Main Tray Menu";
const TRAY_SHORTCUT_GUIDE_HIDE_FROM_MAIN_LABEL: &str = "Hide shortcut guide ⇧ from here";
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
        // On Windows, tray icons sit on the taskbar. In Windows' Custom
        // personalization mode the taskbar theme can differ from the app
        // theme, so the window theme alone may select an invisible icon.
        #[cfg(target_os = "windows")]
        if let Some(theme) = windows_taskbar_theme() {
            return theme;
        }

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

#[cfg(target_os = "windows")]
fn windows_taskbar_theme() -> Option<AppTheme> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let personalize = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")
        .ok()?;
    let system_uses_light: u32 = personalize.get_value("SystemUsesLightTheme").ok()?;

    Some(if system_uses_light == 1 {
        AppTheme::Light
    } else {
        AppTheme::Dark
    })
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

pub fn set_tray_state(app: &AppHandle, state: TrayIconState) {
    sync_tray_with(app, |inner| inner.icon_state = state, None);
}

pub fn change_tray_icon(app: &AppHandle, state: TrayIconState) {
    set_tray_state(app, state);
}

/// Re-applies the current state when the appearance changed without changing
/// whether the app is idle, recording, or transcribing.
pub fn refresh_tray_icon(app: &AppHandle) {
    sync_tray(app, None);
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
    sync_tray_with(app, |inner| inner.icon_state = *state, locale);
}

pub fn refresh_tray_menu(app: &AppHandle, locale: Option<&str>) {
    sync_tray(app, locale);
}

fn sync_tray(app: &AppHandle, locale: Option<&str>) {
    sync_tray_with(app, |_| {}, locale);
}

/// Records the latest desired tray snapshot and schedules one main-thread
/// apply. Concurrent requests are coalesced, and an older slow snapshot can
/// never overwrite a newer request.
fn sync_tray_with(app: &AppHandle, update: impl FnOnce(&mut TrayInner), locale: Option<&str>) {
    let Some(state) = app.try_state::<TrayState>() else {
        return;
    };

    let (seq, icon_state) = {
        let mut inner = state.lock();
        update(&mut inner);
        inner.next_seq += 1;
        (inner.next_seq, inner.icon_state)
    };

    // Early callbacks may arrive before the native tray is built. The icon
    // state remains recorded and the startup sync will apply it later.
    if app.try_state::<TrayIcon>().is_none() {
        return;
    }

    let desired = compute_desired(app, icon_state, locale);
    let needs_icon = !state.lock().icons.contains_key(desired.icon_path);
    let loaded_icon = if needs_icon {
        match load_tray_icon(
            app.path()
                .resolve(desired.icon_path, tauri::path::BaseDirectory::Resource),
        ) {
            Ok(image) => Some(image),
            Err(err) => {
                error!("Failed to load tray icon '{}': {}", desired.icon_path, err);
                None
            }
        }
    } else {
        None
    };

    let schedule = {
        let mut inner = state.lock();
        if let Some(image) = loaded_icon {
            inner.icons.insert(desired.icon_path, image);
        }
        if seq < inner.desired_seq {
            trace!(
                "Tray sync request {} was superseded by {}",
                seq,
                inner.desired_seq
            );
            return;
        }
        inner.desired = Some(desired);
        inner.desired_seq = seq;
        !std::mem::replace(&mut inner.pending, true)
    };

    if schedule {
        post_tray_apply(app);
    }
}

fn compute_desired(
    app: &AppHandle,
    icon_state: TrayIconState,
    locale_override: Option<&str>,
) -> TrayDesired {
    let settings = settings::get_settings(app);
    let available_models = app.state::<Arc<ModelManager>>().get_available_models();
    let selected_local_model_name = available_models
        .iter()
        .find(|model| model.id == settings.selected_model)
        .map(|model| model.name.clone());
    let mut downloaded_local_models: Vec<_> = available_models
        .into_iter()
        .filter(|model| model.is_downloaded)
        .map(|model| TrayModelItem {
            id: model.id,
            name: model.name,
        })
        .collect();
    downloaded_local_models.sort_by(|left, right| left.name.cmp(&right.name));

    let microphones = match audio::get_available_microphones_blocking() {
        Ok(devices) => devices
            .into_iter()
            .map(|device| TrayMicrophoneItem {
                index: device.index,
                name: device.name,
                is_default: device.is_default,
            })
            .collect(),
        Err(err) => {
            warn!("Failed to list microphones for tray menu: {}", err);
            vec![TrayMicrophoneItem {
                index: "default".to_string(),
                name: TRAY_MICROPHONE_DEFAULT_LABEL.to_string(),
                is_default: true,
            }]
        }
    };

    let shortcut_items = if !icon_state.is_busy() && settings.show_tray_shortcut_guide {
        crate::hotkey_guide::build_hotkey_guide_sections(&settings)
            .into_iter()
            .flat_map(|section| section.bindings)
            .map(|binding| TrayShortcutItem {
                id: binding.id,
                label: shortcut_guide_item_label(&binding.name, &binding.current_binding),
            })
            .collect()
    } else {
        Vec::new()
    };

    TrayDesired {
        icon_path: get_icon_path(get_current_theme(app), icon_state),
        menu: MenuInputs {
            busy: icon_state.is_busy(),
            locale: locale_override
                .map(str::to_string)
                .unwrap_or_else(|| settings.app_language.clone()),
            update_checks_enabled: settings.update_checks_enabled,
            transcription_provider: settings.transcription_provider,
            selected_model: settings.selected_model,
            selected_local_model_name,
            selected_microphone: settings.selected_microphone,
            remote_provider_preset: settings.remote_stt.provider_preset,
            remote_model_id: settings.remote_stt.model_id,
            soniox_model: settings.soniox_model,
            deepgram_model: settings.deepgram_model,
            show_shortcut_guide: settings.show_tray_shortcut_guide,
            show_shortcut_guide_in_main_menu: settings.show_tray_shortcut_guide_in_main_menu,
            model_loaded: app.state::<Arc<TranscriptionManager>>().is_model_loaded(),
            downloaded_local_models,
            microphones,
            shortcut_items,
        },
    }
}

fn post_tray_apply(app: &AppHandle) {
    let handle = app.clone();
    if let Err(err) = app.run_on_main_thread(move || apply_tray_on_main(&handle)) {
        error!("Failed to dispatch tray update to the main thread: {}", err);
        if let Some(state) = app.try_state::<TrayState>() {
            state.lock().pending = false;
        }
    }
}

/// The only code path that mutates the native tray icon or menu.
fn apply_tray_on_main(app: &AppHandle) {
    let Some(state) = app.try_state::<TrayState>() else {
        return;
    };
    let Some(tray) = app.try_state::<TrayIcon>() else {
        return;
    };

    let started = Instant::now();
    let (desired, icon, icon_changed, menu_changed) = {
        let mut inner = state.lock();
        inner.pending = false;
        let Some(desired) = inner.desired.clone() else {
            return;
        };
        let icon_changed = inner.applied_icon != Some(desired.icon_path);
        let menu_changed = inner.applied_menu.as_ref() != Some(&desired.menu);
        if !icon_changed && !menu_changed {
            return;
        }
        let icon = inner.icons.get(desired.icon_path).cloned();
        (desired, icon, icon_changed, menu_changed)
    };

    let mut icon_applied = false;
    if icon_changed {
        match icon {
            Some(image) => match tray.set_icon_with_as_template(Some(image), true) {
                Ok(()) => icon_applied = true,
                Err(err) => error!("Failed to apply tray icon '{}': {}", desired.icon_path, err),
            },
            None => error!("Tray icon '{}' is not loaded", desired.icon_path),
        }
    }

    let mut menu_applied = false;
    if menu_changed {
        match build_tray_menu(app, &desired.menu) {
            Ok((menu, tooltip)) => match tray.set_menu(Some(menu)) {
                Ok(()) => {
                    menu_applied = true;
                    if let Err(err) = tray.set_tooltip(Some(tooltip)) {
                        error!("Failed to set tray tooltip: {}", err);
                    }
                }
                Err(err) => error!("Failed to set tray menu: {}", err),
            },
            Err(err) => error!("Failed to build tray menu: {}", err),
        }
    }

    {
        let mut inner = state.lock();
        if icon_applied {
            inner.applied_icon = Some(desired.icon_path);
        }
        if menu_applied {
            inner.applied_menu = Some(desired.menu.clone());
        }
    }

    debug!(
        "Tray apply: icon={}, menu={}, busy={}, took={:?}",
        if icon_changed {
            desired.icon_path
        } else {
            "unchanged"
        },
        if menu_changed { "rebuilt" } else { "unchanged" },
        desired.menu.busy,
        started.elapsed()
    );
}

fn load_tray_icon(resolved_icon_path: tauri::Result<PathBuf>) -> tauri::Result<Image<'static>> {
    let resolved_icon_path = resolved_icon_path?;
    Image::from_path(&resolved_icon_path).map(Image::to_owned)
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

fn build_tray_menu(
    app: &AppHandle,
    inputs: &MenuInputs,
) -> Result<(Menu<tauri::Wry>, String), Box<dyn std::error::Error>> {
    let strings = get_tray_translations(Some(inputs.locale.clone()));
    #[cfg(debug_assertions)]
    let _ = &strings.restart_troubleshoot;

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
        inputs.update_checks_enabled,
        None::<&str>,
    )?;
    #[cfg(not(debug_assertions))]
    let restart_troubleshoot_i = MenuItem::with_id(
        app,
        "restart_troubleshoot",
        &strings.restart_troubleshoot,
        true,
        None::<&str>,
    )?;
    let copy_last_transcript_i = MenuItem::with_id(
        app,
        "copy_last_transcript",
        &strings.copy_last_transcript,
        true,
        None::<&str>,
    )?;
    let local_model_selected = inputs.transcription_provider == TranscriptionProvider::Local
        && !inputs.selected_model.trim().is_empty();
    let can_unload_model = inputs.model_loaded || local_model_selected;
    let unload_model_label = if can_unload_model {
        TRAY_UNLOAD_LOCAL_MODEL_LABEL
    } else if inputs.transcription_provider != TranscriptionProvider::Local {
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
        can_unload_model,
        None::<&str>,
    )?;
    let model_menu_label = build_model_menu_label(inputs, &strings.model);
    let quit_i = MenuItem::with_id(app, "quit", &strings.quit, true, quit_accelerator)?;
    let separator = || PredefinedMenuItem::separator(app);

    let menu = Menu::new(app)?;

    menu.append(&version_i)?;
    if inputs.busy {
        let cancel_i = MenuItem::with_id(app, "cancel", &strings.cancel, true, None::<&str>)?;
        menu.append(&separator()?)?;
        menu.append(&cancel_i)?;
    }

    menu.append(&separator()?)?;
    append_microphone_items(&menu, app, inputs)?;
    menu.append(&separator()?)?;
    menu.append(&copy_last_transcript_i)?;

    if !inputs.busy {
        let model_submenu = build_model_submenu(app, &model_menu_label, inputs)?;
        menu.append(&separator()?)?;
        menu.append(&model_submenu)?;
        menu.append(&unload_model_i)?;
        if inputs.show_shortcut_guide {
            if inputs.show_shortcut_guide_in_main_menu {
                append_shortcut_guide_main_menu_items(&menu, app, inputs)?;
            } else if let Some(guide_submenu) = build_shortcut_guide_submenu(app, inputs)? {
                menu.append(&separator()?)?;
                menu.append(&guide_submenu)?;
            }
        }
    }

    menu.append(&separator()?)?;
    menu.append(&settings_i)?;
    menu.append(&check_updates_i)?;
    #[cfg(not(debug_assertions))]
    menu.append(&restart_troubleshoot_i)?;
    menu.append(&separator()?)?;
    menu.append(&quit_i)?;

    Ok((menu, version_label))
}

fn build_model_menu_label(inputs: &MenuInputs, fallback_label: &str) -> String {
    let fallback_label = if fallback_label.is_empty() {
        "Model"
    } else {
        fallback_label
    };

    match inputs.transcription_provider {
        TranscriptionProvider::Local => {
            let selected_name = inputs
                .selected_local_model_name
                .clone()
                .unwrap_or_else(|| fallback_label.to_string());
            format!("{TRAY_MODEL_LOCAL_LABEL}: {selected_name}")
        }
        TranscriptionProvider::RemoteOpenAiCompatible => {
            let provider_label = match inputs.remote_provider_preset.as_str() {
                REMOTE_STT_PRESET_GROQ => "Groq",
                REMOTE_STT_PRESET_OPENAI => "OpenAI",
                REMOTE_STT_PRESET_CUSTOM => TRAY_MODEL_CUSTOM_SUFFIX,
                _ => TRAY_MODEL_REMOTE_LABEL,
            };
            format!("{provider_label}: {}", inputs.remote_model_id)
        }
        TranscriptionProvider::RemoteSoniox => {
            format!("{TRAY_MODEL_SONIOX_LABEL}: {}", inputs.soniox_model)
        }
        TranscriptionProvider::RemoteDeepgram => {
            format!("{TRAY_MODEL_DEEPGRAM_LABEL}: {}", inputs.deepgram_model)
        }
    }
}

fn append_microphone_items(
    menu: &Menu<tauri::Wry>,
    app: &AppHandle,
    inputs: &MenuInputs,
) -> Result<(), Box<dyn std::error::Error>> {
    let header_item = MenuItem::with_id(
        app,
        TRAY_MICROPHONE_HEADER_ID,
        TRAY_MICROPHONE_HEADER_LABEL,
        false,
        None::<&str>,
    )?;
    menu.append(&header_item)?;

    let selected_microphone = inputs.selected_microphone.as_deref();

    let missing_selected_microphone = selected_microphone.filter(|selected_name| {
        !inputs
            .microphones
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

    for device in inputs
        .microphones
        .iter()
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
    inputs: &MenuInputs,
) -> Result<Submenu<tauri::Wry>, Box<dyn std::error::Error>> {
    let submenu = Submenu::with_id(app, TRAY_MODEL_SUBMENU_ID, label, true)?;
    append_local_model_items(&submenu, app, inputs)?;
    submenu.append(&PredefinedMenuItem::separator(app)?)?;
    append_remote_openai_model_items(&submenu, app, inputs)?;
    submenu.append(&PredefinedMenuItem::separator(app)?)?;
    append_soniox_model_items(&submenu, app, inputs)?;
    submenu.append(&PredefinedMenuItem::separator(app)?)?;
    append_deepgram_model_items(&submenu, app, inputs)?;

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
    inputs: &MenuInputs,
) -> Result<(), Box<dyn std::error::Error>> {
    append_submenu_header(
        submenu,
        app,
        TRAY_MODEL_LOCAL_HEADER_ID,
        TRAY_MODEL_LOCAL_LABEL,
    )?;

    if inputs.downloaded_local_models.is_empty() {
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

    for model in &inputs.downloaded_local_models {
        let item = CheckMenuItem::with_id(
            app,
            model_menu_id(TRAY_MODEL_PREFIX_LOCAL, &model.id),
            &model.name,
            true,
            inputs.transcription_provider == TranscriptionProvider::Local
                && model.id == inputs.selected_model,
            None::<&str>,
        )?;
        submenu.append(&item)?;
    }

    Ok(())
}

fn append_remote_openai_model_items(
    submenu: &Submenu<tauri::Wry>,
    app: &AppHandle,
    inputs: &MenuInputs,
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
            "gpt-transcribe".to_string(),
            "OpenAI: gpt-transcribe (file / post-recording)".to_string(),
        ),
        (
            REMOTE_STT_PRESET_OPENAI.to_string(),
            "gpt-live-transcribe".to_string(),
            "OpenAI: gpt-live-transcribe (live)".to_string(),
        ),
        (
            REMOTE_STT_PRESET_OPENAI.to_string(),
            "gpt-realtime-2".to_string(),
            "OpenAI: gpt-realtime-2 · Legacy STT Hack".to_string(),
        ),
        (
            REMOTE_STT_PRESET_OPENAI.to_string(),
            REMOTE_STT_OPENAI_DEFAULT_MODEL.to_string(),
            "OpenAI: gpt-realtime-2.1 · Latest STT Hack".to_string(),
        ),
        (
            REMOTE_STT_PRESET_OPENAI.to_string(),
            "gpt-realtime-whisper".to_string(),
            "OpenAI: gpt-realtime-whisper · Legacy".to_string(),
        ),
        (
            REMOTE_STT_PRESET_OPENAI.to_string(),
            "gpt-realtime-translate".to_string(),
            "OpenAI: gpt-realtime-translate".to_string(),
        ),
    ];

    let current_model = inputs.remote_model_id.trim();
    let current_preset = match inputs.remote_provider_preset.trim() {
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
            inputs.transcription_provider == TranscriptionProvider::RemoteOpenAiCompatible
                && provider_preset == inputs.remote_provider_preset
                && model_id == inputs.remote_model_id,
            None::<&str>,
        )?;
        submenu.append(&item)?;
    }

    Ok(())
}

fn append_soniox_model_items(
    submenu: &Submenu<tauri::Wry>,
    app: &AppHandle,
    inputs: &MenuInputs,
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
        ("stt-async-v5".to_string(), "stt-async-v5".to_string()),
    ];
    let current_model = inputs.soniox_model.trim();
    if !current_model.is_empty() && !models.iter().any(|(model_id, _)| model_id == current_model) {
        models.push((current_model.to_string(), current_model.to_string()));
    }

    append_provider_model_items(
        submenu,
        app,
        TRAY_MODEL_PREFIX_SONIOX,
        TranscriptionProvider::RemoteSoniox,
        &inputs.transcription_provider,
        &inputs.soniox_model,
        models,
    )
}

fn append_deepgram_model_items(
    submenu: &Submenu<tauri::Wry>,
    app: &AppHandle,
    inputs: &MenuInputs,
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
    let current_model = inputs.deepgram_model.trim();
    if !current_model.is_empty() && !models.iter().any(|(model_id, _)| model_id == current_model) {
        models.push((current_model.to_string(), current_model.to_string()));
    }

    append_provider_model_items(
        submenu,
        app,
        TRAY_MODEL_PREFIX_DEEPGRAM,
        TranscriptionProvider::RemoteDeepgram,
        &inputs.transcription_provider,
        &inputs.deepgram_model,
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
    inputs: &MenuInputs,
) -> Result<Option<Submenu<tauri::Wry>>, Box<dyn std::error::Error>> {
    if inputs.shortcut_items.is_empty() {
        return Ok(None);
    }

    let submenu = Submenu::with_id(app, "tray_shortcut_guide", TRAY_SHORTCUT_GUIDE_LABEL, true)?;

    for shortcut in &inputs.shortcut_items {
        let item = MenuItem::with_id(
            app,
            format!("tray_shortcut_guide_item::{}", shortcut.id),
            &shortcut.label,
            false,
            None::<&str>,
        )?;
        submenu.append(&item)?;
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
    inputs: &MenuInputs,
) -> Result<(), Box<dyn std::error::Error>> {
    if inputs.shortcut_items.is_empty() {
        return Ok(());
    }

    menu.append(&PredefinedMenuItem::separator(app)?)?;

    let title = MenuItem::with_id(
        app,
        "tray_shortcut_guide_main_header",
        TRAY_SHORTCUT_GUIDE_LABEL,
        false,
        None::<&str>,
    )?;
    menu.append(&title)?;

    for shortcut in &inputs.shortcut_items {
        let item = MenuItem::with_id(
            app,
            format!("tray_shortcut_guide_main_item::{}", shortcut.id),
            &shortcut.label,
            false,
            None::<&str>,
        )?;
        menu.append(&item)?;
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

    let _clipboard_guard = match crate::clipboard::lock_clipboard_transaction(
        "copying the last transcript from the tray",
    ) {
        Ok(guard) => guard,
        Err(err) => {
            error!("{}", err);
            return;
        }
    };
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
    fn recording_and_transcribing_share_the_busy_menu_shape() {
        assert!(TrayIconState::Recording.is_busy());
        assert!(TrayIconState::Transcribing.is_busy());
        assert!(!TrayIconState::Idle.is_busy());
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
                "{TRAY_MODEL_MENU_PREFIX}remote_openai_compatible::openai::gpt-realtime-2"
            )),
            Some(TrayModelSelection::RemoteOpenAiCompatible {
                provider_preset: "openai".to_string(),
                model_id: "gpt-realtime-2".to_string(),
            })
        );
        assert_eq!(
            parse_model_menu_selection(&format!(
                "{TRAY_MODEL_MENU_PREFIX}remote_soniox::stt-rt-v5"
            )),
            Some(TrayModelSelection::RemoteSoniox("stt-rt-v5".to_string()))
        );
        assert_eq!(parse_model_menu_selection("some-other-id"), None);
    }
}
