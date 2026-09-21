use crate::managers::gemini_realtime::{
    GEMINI_LIVE_DEFAULT_MODEL, GEMINI_LIVE_GOOGLE_DEFAULT_MODEL,
};
use crate::managers::history::{HistoryEntry, HistoryManager};
use crate::managers::model::ModelManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::TranscriptionProvider;
use crate::tray_i18n::get_tray_translations;
use crate::url_security::{
    REMOTE_STT_GOOGLE_DEFAULT_MODEL, REMOTE_STT_GROQ_DEFAULT_MODEL,
    REMOTE_STT_OPENAI_DEFAULT_MODEL, REMOTE_STT_PRESET_CUSTOM, REMOTE_STT_PRESET_GOOGLE,
    REMOTE_STT_PRESET_GROQ, REMOTE_STT_PRESET_OPENAI, REMOTE_STT_PRESET_VERCEL,
    REMOTE_STT_VERCEL_DEFAULT_MODEL,
};
use crate::{commands::audio, settings};
use log::{debug, error, info, trace, warn};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
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

static MAIN_THREAD_POST_PENDING: AtomicBool = AtomicBool::new(false);

/// Blink settings captured before the tray lock is taken. The settings store
/// must never be read while holding the tray lock.
#[derive(Clone, Copy, Debug, PartialEq)]
struct BlinkPolicy {
    tray_visible: bool,
    enabled: bool,
    on_recording: bool,
    on_processing: bool,
    frequency_hz: f64,
}

impl BlinkPolicy {
    fn from_settings(settings: &settings::AppSettings) -> Self {
        Self {
            tray_visible: settings.show_tray_icon,
            enabled: settings.tray_icon_blinking_enabled,
            on_recording: settings.tray_icon_blink_on_recording,
            on_processing: settings.tray_icon_blink_on_processing,
            frequency_hz: settings::normalize_tray_icon_blink_frequency_hz(
                settings.tray_icon_blink_frequency_hz,
            ),
        }
    }

    fn load(app: &AppHandle) -> Self {
        Self::from_settings(&settings::get_settings(app))
    }

    fn applies_to(&self, state: TrayIconState) -> bool {
        self.tray_visible
            && self.enabled
            && match state {
                TrayIconState::Recording => self.on_recording,
                TrayIconState::Transcribing => self.on_processing,
                TrayIconState::Idle => false,
            }
    }

    fn half_period(&self) -> Duration {
        Duration::from_secs_f64(0.5 / self.frequency_hz)
    }
}

/// A blink loop claimed under the tray lock and started once it is released.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BlinkPlan {
    generation: u64,
    half_period: Duration,
}

/// Ordering claimed for a tray sync whose (slow) snapshot is computed later.
/// Dropping a ticket without committing it leaves the tray stale until the
/// next sync, so callers should commit as soon as they leave their lock.
#[must_use = "commit the ticket with `commit_tray_sync`"]
pub struct TraySyncTicket {
    seq: u64,
    icon_state: TrayIconState,
}

fn spawn_blink_loop(app: &AppHandle, plan: BlinkPlan) {
    let app = app.clone();
    std::thread::spawn(move || {
        let mut show_recording_icon = false;
        loop {
            std::thread::sleep(plan.half_period);
            if !blink_is_current(&app, plan.generation) {
                break;
            }

            show_recording_icon = !show_recording_icon;
            // Alternates between standard app logo and recording "ear" icon
            let frame = if show_recording_icon {
                TrayIconState::Recording
            } else {
                TrayIconState::Idle
            };

            if MAIN_THREAD_POST_PENDING
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                let h = app.clone();
                if app
                    .run_on_main_thread(move || {
                        MAIN_THREAD_POST_PENDING.store(false, Ordering::SeqCst);
                        apply_blink_frame_on_main(&h, plan.generation, frame);
                    })
                    .is_err()
                {
                    MAIN_THREAD_POST_PENDING.store(false, Ordering::SeqCst);
                }
            }
        }
    });
}

fn blink_is_current(app: &AppHandle, generation: u64) -> bool {
    app.try_state::<TrayState>()
        .map(|state| state.lock().blink_is_current(generation))
        .unwrap_or(false)
}

fn apply_blink_frame_on_main(app: &AppHandle, generation: u64, frame: TrayIconState) {
    let Some(tray_state) = app.try_state::<TrayState>() else {
        return;
    };
    let Some(tray) = app.try_state::<TrayIcon>() else {
        return;
    };

    let theme = get_current_theme(app);
    let icon_path = get_icon_path(theme, frame);

    let image = {
        let mut inner = tray_state.lock();
        // Re-checked under the lock: a frame queued before the loop was
        // retired must not overwrite the static icon applied after it.
        if !inner.blink_is_current(generation) {
            return;
        }
        inner.applied_icon = None;
        if let Some(img) = inner.icons.get(icon_path).cloned() {
            img
        } else if let Ok(img) = load_tray_icon(
            app.path()
                .resolve(icon_path, tauri::path::BaseDirectory::Resource),
        ) {
            inner.icons.insert(icon_path, img.clone());
            img
        } else {
            return;
        }
    };

    let _ = tray.set_icon_with_as_template(Some(image), true);
}

pub fn set_tray_state(app: &AppHandle, state: TrayIconState) {
    if let Some(ticket) = claim_tray_state(app, state, &settings::get_settings(app)) {
        commit_tray_sync(app, ticket);
    }
}

pub fn change_tray_icon(app: &AppHandle, state: TrayIconState) {
    set_tray_state(app, state);
}

/// Records the new icon state and settles the blink loop without touching
/// the native tray. Cheap enough to call while holding another lock that
/// decides whether the transition is still valid; the returned ticket must
/// then be committed after that lock is released. Settings are passed in
/// because loading them reads the store file from disk, which callers should
/// do before taking their lock.
pub fn claim_tray_state(
    app: &AppHandle,
    state: TrayIconState,
    settings: &settings::AppSettings,
) -> Option<TraySyncTicket> {
    let policy = BlinkPolicy::from_settings(settings);
    claim_tray_sync(app, |inner| inner.icon_state = state, Some(policy))
}

/// Re-applies the current state when the appearance changed without changing
/// whether the app is idle, recording, or transcribing.
pub fn refresh_tray_icon(app: &AppHandle) {
    let policy = BlinkPolicy::load(app);
    if let Some(ticket) = claim_tray_sync(app, |_| {}, Some(policy)) {
        commit_tray_sync(app, ticket);
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
    webviews_disabled: bool,
    show_speech_only_mode_in_tray: bool,
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
    /// Identifies the blink loop allowed to touch the tray. Changed only
    /// together with `icon_state`, under the same lock, so the last writer of
    /// the state is always the last to decide whether the tray blinks.
    blink_generation: u64,
}

impl TrayInner {
    /// Retires the running blink loop and, when the current state should
    /// blink, claims a new generation for its replacement.
    fn transition_blink(&mut self, policy: &BlinkPolicy) -> Option<BlinkPlan> {
        self.blink_generation += 1;
        policy.applies_to(self.icon_state).then(|| BlinkPlan {
            generation: self.blink_generation,
            half_period: policy.half_period(),
        })
    }

    /// A loop keeps running only while it owns the generation and the tray
    /// is still busy. The second check is defense in depth: it stops any loop
    /// that outlived its state without waiting for the next transition.
    fn blink_is_current(&self, generation: u64) -> bool {
        self.blink_generation == generation && self.icon_state.is_busy()
    }

    /// Claims the next sync ordering slot for the current icon state.
    fn claim_seq(&mut self) -> TraySyncTicket {
        self.next_seq += 1;
        TraySyncTicket {
            seq: self.next_seq,
            icon_state: self.icon_state,
        }
    }

    /// Stores the snapshot for `ticket` unless a newer ticket already landed.
    /// Returns whether a main-thread apply must be posted.
    fn record_desired(&mut self, ticket: &TraySyncTicket, desired: TrayDesired) -> Option<bool> {
        if ticket.seq < self.desired_seq {
            trace!(
                "Tray sync request {} was superseded by {}",
                ticket.seq,
                self.desired_seq
            );
            return None;
        }
        self.desired = Some(desired);
        self.desired_seq = ticket.seq;
        Some(!std::mem::replace(&mut self.pending, true))
    }
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
            blink_generation: 0,
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
    let policy = BlinkPolicy::load(app);
    if let Some(ticket) = claim_tray_sync(app, |inner| inner.icon_state = *state, Some(policy)) {
        commit_tray_sync_with_locale(app, ticket, locale);
    }
}

pub fn refresh_tray_menu(app: &AppHandle, locale: Option<&str>) {
    if let Some(ticket) = claim_tray_sync(app, |_| {}, None) {
        commit_tray_sync_with_locale(app, ticket, locale);
    }
}

/// Applies `update` and claims the ordering slot for the resulting state in
/// one critical section. With a policy, the blink loop is settled in the same
/// section, so no stale transition can restart blinking after a newer one
/// stopped it. Any write to `icon_state` must pass a policy.
fn claim_tray_sync(
    app: &AppHandle,
    update: impl FnOnce(&mut TrayInner),
    blink_policy: Option<BlinkPolicy>,
) -> Option<TraySyncTicket> {
    let state = app.try_state::<TrayState>()?;

    let (ticket, blink_plan) = {
        let mut inner = state.lock();
        update(&mut inner);
        let blink_plan = blink_policy
            .as_ref()
            .and_then(|policy| inner.transition_blink(policy));
        (inner.claim_seq(), blink_plan)
    };

    if let Some(plan) = blink_plan {
        spawn_blink_loop(app, plan);
    }

    Some(ticket)
}

/// Computes and records the tray snapshot for a claimed ticket and schedules
/// one main-thread apply. Concurrent requests are coalesced, and an older slow
/// snapshot can never overwrite a newer request.
pub fn commit_tray_sync(app: &AppHandle, ticket: TraySyncTicket) {
    commit_tray_sync_with_locale(app, ticket, None);
}

fn commit_tray_sync_with_locale(app: &AppHandle, ticket: TraySyncTicket, locale: Option<&str>) {
    let Some(state) = app.try_state::<TrayState>() else {
        return;
    };

    // Early callbacks may arrive before the native tray is built. The icon
    // state remains recorded and the startup sync will apply it later.
    if app.try_state::<TrayIcon>().is_none() {
        return;
    }

    let desired = compute_desired(app, ticket.icon_state, locale);
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
        inner.record_desired(&ticket, desired)
    };

    if schedule == Some(true) {
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
            .filter(|binding| {
                !crate::webview_mode::webviews_disabled()
                    || crate::webview_mode::speech_only_shortcut_allowed(&binding.id)
            })
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
            webviews_disabled: crate::webview_mode::webviews_disabled(),
            show_speech_only_mode_in_tray: settings.show_speech_only_mode_in_tray,
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

    let elapsed = started.elapsed();
    let message = format!(
        "Tray apply: icon={}, menu={}, busy={}, took={elapsed:?}",
        if icon_changed {
            desired.icon_path
        } else {
            "unchanged"
        },
        if menu_changed { "rebuilt" } else { "unchanged" },
        desired.menu.busy
    );
    if elapsed >= std::time::Duration::from_millis(100) {
        info!("{message}");
    } else {
        debug!("{message}");
    }
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
    let settings_label = if inputs.webviews_disabled {
        if strings.settings_requires_webview.is_empty() {
            "Switch to full interface and restart"
        } else {
            &strings.settings_requires_webview
        }
    } else {
        &strings.settings
    };
    let settings_i = MenuItem::with_id(
        app,
        "settings",
        settings_label,
        true,
        settings_accelerator,
    )?;
    let enter_speech_only_i = MenuItem::with_id(
        app,
        "enter_speech_only_mode",
        if strings.enter_speech_only_mode.is_empty() {
            "Restart in speech-only mode"
        } else {
            &strings.enter_speech_only_mode
        },
        true,
        None::<&str>,
    )?;
    let check_updates_i = MenuItem::with_id(
        app,
        "check_updates",
        &strings.check_updates,
        inputs.update_checks_enabled && !inputs.webviews_disabled,
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
    if should_show_enter_speech_only_mode(
        inputs.webviews_disabled,
        inputs.show_speech_only_mode_in_tray,
    ) {
        menu.append(&enter_speech_only_i)?;
    }
    menu.append(&settings_i)?;
    menu.append(&check_updates_i)?;
    #[cfg(not(debug_assertions))]
    menu.append(&restart_troubleshoot_i)?;
    menu.append(&separator()?)?;
    menu.append(&quit_i)?;

    Ok((menu, version_label))
}

fn should_show_enter_speech_only_mode(
    webviews_disabled: bool,
    show_speech_only_mode_in_tray: bool,
) -> bool {
    !webviews_disabled && show_speech_only_mode_in_tray
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
                REMOTE_STT_PRESET_VERCEL => "Vercel",
                REMOTE_STT_PRESET_GOOGLE => "Google",
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
            REMOTE_STT_PRESET_VERCEL.to_string(),
            REMOTE_STT_VERCEL_DEFAULT_MODEL.to_string(),
            "Vercel: Gemini 3.5 Transcribe".to_string(),
        ),
        (
            REMOTE_STT_PRESET_VERCEL.to_string(),
            GEMINI_LIVE_DEFAULT_MODEL.to_string(),
            "Vercel: Gemini 3.5 Transcribe Live (Realtime)".to_string(),
        ),
        (
            REMOTE_STT_PRESET_GOOGLE.to_string(),
            REMOTE_STT_GOOGLE_DEFAULT_MODEL.to_string(),
            "Google: Gemini 3.5 Transcribe · Experimental".to_string(),
        ),
        (
            REMOTE_STT_PRESET_GOOGLE.to_string(),
            GEMINI_LIVE_GOOGLE_DEFAULT_MODEL.to_string(),
            "Google: Gemini 3.5 Transcribe Live (Realtime)".to_string(),
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
        parse_model_menu_selection, should_show_enter_speech_only_mode, tray_tooltip, AppTheme,
        BlinkPolicy, MenuInputs, TrayDesired, TrayIconState, TrayModelSelection, TrayState,
        TRAY_MICROPHONE_DEFAULT_ID, TRAY_MICROPHONE_MENU_PREFIX, TRAY_MICROPHONE_MISSING_ID,
        TRAY_MODEL_MENU_PREFIX,
    };
    use crate::managers::history::HistoryEntry;
    use crate::settings::TranscriptionProvider;
    use std::time::Duration;

    fn blink_everywhere() -> BlinkPolicy {
        BlinkPolicy {
            tray_visible: true,
            enabled: true,
            on_recording: true,
            on_processing: true,
            frequency_hz: 2.0,
        }
    }

    fn desired_for(icon_state: TrayIconState) -> TrayDesired {
        TrayDesired {
            icon_path: get_icon_path(AppTheme::Dark, icon_state),
            menu: MenuInputs {
                busy: icon_state.is_busy(),
                webviews_disabled: false,
                show_speech_only_mode_in_tray: false,
                locale: "en".to_string(),
                update_checks_enabled: false,
                transcription_provider: TranscriptionProvider::Local,
                selected_model: String::new(),
                selected_local_model_name: None,
                selected_microphone: None,
                remote_provider_preset: String::new(),
                remote_model_id: String::new(),
                soniox_model: String::new(),
                deepgram_model: String::new(),
                show_shortcut_guide: false,
                show_shortcut_guide_in_main_menu: false,
                model_loaded: false,
                downloaded_local_models: Vec::new(),
                microphones: Vec::new(),
                shortcut_items: Vec::new(),
            },
        }
    }

    #[test]
    fn blink_policy_only_applies_to_busy_states_it_is_enabled_for() {
        let policy = blink_everywhere();
        assert!(!policy.applies_to(TrayIconState::Idle));
        assert!(policy.applies_to(TrayIconState::Recording));
        assert!(policy.applies_to(TrayIconState::Transcribing));

        let recording_only = BlinkPolicy {
            on_processing: false,
            ..policy
        };
        assert!(recording_only.applies_to(TrayIconState::Recording));
        assert!(!recording_only.applies_to(TrayIconState::Transcribing));

        let disabled = BlinkPolicy {
            enabled: false,
            ..policy
        };
        assert!(!disabled.applies_to(TrayIconState::Recording));

        let hidden_tray = BlinkPolicy {
            tray_visible: false,
            ..policy
        };
        assert!(!hidden_tray.applies_to(TrayIconState::Recording));

        assert_eq!(policy.half_period(), Duration::from_millis(250));
    }

    #[test]
    fn transition_to_idle_retires_the_running_blink_loop() {
        let state = TrayState::new();
        let mut inner = state.lock();
        let policy = blink_everywhere();

        inner.icon_state = TrayIconState::Recording;
        let recording = inner
            .transition_blink(&policy)
            .expect("recording should blink");
        assert!(inner.blink_is_current(recording.generation));

        inner.icon_state = TrayIconState::Idle;
        assert!(inner.transition_blink(&policy).is_none());
        assert!(!inner.blink_is_current(recording.generation));
    }

    #[test]
    fn every_transition_retires_the_previous_loop_even_when_the_new_one_blinks() {
        let state = TrayState::new();
        let mut inner = state.lock();
        let policy = blink_everywhere();

        inner.icon_state = TrayIconState::Recording;
        let recording = inner.transition_blink(&policy).expect("blinks");
        inner.icon_state = TrayIconState::Transcribing;
        let transcribing = inner.transition_blink(&policy).expect("blinks");

        assert_ne!(recording.generation, transcribing.generation);
        assert!(!inner.blink_is_current(recording.generation));
        assert!(inner.blink_is_current(transcribing.generation));
    }

    #[test]
    fn a_loop_never_survives_an_idle_icon_state() {
        // Defense in depth: even if a generation somehow stayed current, an
        // idle tray must stop any loop within one half period.
        let state = TrayState::new();
        let mut inner = state.lock();
        let policy = blink_everywhere();

        inner.icon_state = TrayIconState::Recording;
        let plan = inner.transition_blink(&policy).expect("blinks");
        inner.icon_state = TrayIconState::Idle;
        assert!(!inner.blink_is_current(plan.generation));
    }

    #[test]
    fn blink_decision_follows_the_last_state_writer_regardless_of_interleaving() {
        // Models the stop path: the hotkey thread writes Transcribing and the
        // transcription task writes Idle. Whatever order the two claims take
        // under the lock, only the last claimed state decides the blink loop.
        for order in [
            [TrayIconState::Transcribing, TrayIconState::Idle],
            [TrayIconState::Idle, TrayIconState::Transcribing],
        ] {
            let state = TrayState::new();
            let mut inner = state.lock();
            let policy = blink_everywhere();
            let mut plans = Vec::new();
            for icon_state in order {
                inner.icon_state = icon_state;
                plans.push(inner.transition_blink(&policy));
            }

            let (earlier, last) = (plans[0], plans[1]);
            if let Some(earlier) = earlier {
                assert!(!inner.blink_is_current(earlier.generation));
            }
            assert_eq!(last.is_some(), policy.applies_to(inner.icon_state));
            if let Some(last) = last {
                assert!(inner.blink_is_current(last.generation));
            }
        }
    }

    #[test]
    fn a_slow_older_sync_never_overwrites_a_newer_snapshot() {
        // Models the recording start: the Recording ticket is claimed under
        // the session lock, then a stop claims Transcribing, commits first,
        // and the late Recording commit must be discarded.
        let state = TrayState::new();
        let mut inner = state.lock();

        inner.icon_state = TrayIconState::Recording;
        let recording = inner.claim_seq();
        inner.icon_state = TrayIconState::Transcribing;
        let transcribing = inner.claim_seq();
        assert!(recording.seq < transcribing.seq);
        assert_eq!(recording.icon_state, TrayIconState::Recording);
        assert_eq!(transcribing.icon_state, TrayIconState::Transcribing);

        assert_eq!(
            inner.record_desired(&transcribing, desired_for(TrayIconState::Transcribing)),
            Some(true)
        );
        assert_eq!(
            inner.record_desired(&recording, desired_for(TrayIconState::Recording)),
            None
        );
        assert_eq!(
            inner.desired.as_ref().map(|desired| desired.icon_path),
            Some(get_icon_path(AppTheme::Dark, TrayIconState::Transcribing))
        );
    }

    #[test]
    fn newer_syncs_coalesce_into_the_pending_apply() {
        let state = TrayState::new();
        let mut inner = state.lock();

        let first = inner.claim_seq();
        let second = inner.claim_seq();
        assert_eq!(
            inner.record_desired(&first, desired_for(TrayIconState::Idle)),
            Some(true)
        );
        // The apply is already posted; the newer snapshot only replaces it.
        assert_eq!(
            inner.record_desired(&second, desired_for(TrayIconState::Recording)),
            Some(false)
        );
        assert_eq!(inner.desired_seq, second.seq);
    }

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
    fn speech_only_entry_is_opt_in_and_only_shown_in_full_mode() {
        assert!(!should_show_enter_speech_only_mode(false, false));
        assert!(should_show_enter_speech_only_mode(false, true));
        assert!(!should_show_enter_speech_only_mode(true, false));
        assert!(!should_show_enter_speech_only_mode(true, true));
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
