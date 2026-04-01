use handy_keys::{Hotkey, HotkeyId, HotkeyManager, HotkeyState, KeyboardListener};
use log::{debug, error, info};
use serde::Serialize;
use specta::Type;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use tauri::{AppHandle, Emitter, Manager};

use crate::settings::{self, get_settings, ShortcutBinding};

enum ManagerCommand {
    Register {
        binding_id: String,
        hotkey_string: String,
        response: Sender<Result<(), String>>,
    },
    Unregister {
        binding_id: String,
        response: Sender<Result<(), String>>,
    },
    Shutdown,
}

pub struct HandyKeysState {
    command_sender: Mutex<Sender<ManagerCommand>>,
    thread_handle: Mutex<Option<JoinHandle<()>>>,
    registered_bindings: Mutex<HashSet<String>>,
    recording_listener: Mutex<Option<KeyboardListener>>,
    is_recording: AtomicBool,
    recording_running: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct FrontendKeyEvent {
    pub modifiers: Vec<String>,
    pub key: Option<String>,
    pub is_key_down: bool,
    pub hotkey_string: String,
}

impl HandyKeysState {
    pub fn new(app: AppHandle) -> Result<Self, String> {
        let (cmd_tx, cmd_rx) = mpsc::channel::<ManagerCommand>();
        let app_clone = app.clone();
        let thread_handle = thread::spawn(move || {
            Self::manager_thread(cmd_rx, app_clone);
        });

        Ok(Self {
            command_sender: Mutex::new(cmd_tx),
            thread_handle: Mutex::new(Some(thread_handle)),
            registered_bindings: Mutex::new(HashSet::new()),
            recording_listener: Mutex::new(None),
            is_recording: AtomicBool::new(false),
            recording_running: Arc::new(AtomicBool::new(false)),
        })
    }

    fn manager_thread(cmd_rx: Receiver<ManagerCommand>, app: AppHandle) {
        info!("handy-keys manager thread started");

        let manager = match HotkeyManager::new_with_blocking() {
            Ok(manager) => manager,
            Err(err) => {
                error!("Failed to create handy-keys manager: {}", err);
                return;
            }
        };

        let mut binding_to_hotkey: HashMap<String, HotkeyId> = HashMap::new();
        let mut hotkey_to_binding: HashMap<HotkeyId, (String, String)> = HashMap::new();

        loop {
            while let Some(event) = manager.try_recv() {
                if let Some((binding_id, hotkey_string)) = hotkey_to_binding.get(&event.id) {
                    let is_pressed = event.state == HotkeyState::Pressed;
                    crate::shortcut::handle_shortcut_event(
                        &app,
                        binding_id,
                        hotkey_string,
                        is_pressed,
                    );
                }
            }

            match cmd_rx.recv_timeout(std::time::Duration::from_millis(10)) {
                Ok(cmd) => match cmd {
                    ManagerCommand::Register {
                        binding_id,
                        hotkey_string,
                        response,
                    } => {
                        let result = Self::do_register(
                            &manager,
                            &mut binding_to_hotkey,
                            &mut hotkey_to_binding,
                            &binding_id,
                            &hotkey_string,
                        );
                        let _ = response.send(result);
                    }
                    ManagerCommand::Unregister {
                        binding_id,
                        response,
                    } => {
                        let result = Self::do_unregister(
                            &manager,
                            &mut binding_to_hotkey,
                            &mut hotkey_to_binding,
                            &binding_id,
                        );
                        let _ = response.send(result);
                    }
                    ManagerCommand::Shutdown => {
                        info!("handy-keys manager thread shutting down");
                        break;
                    }
                },
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    info!("handy-keys command channel disconnected");
                    break;
                }
            }
        }

        info!("handy-keys manager thread stopped");
    }

    fn do_register(
        manager: &HotkeyManager,
        binding_to_hotkey: &mut HashMap<String, HotkeyId>,
        hotkey_to_binding: &mut HashMap<HotkeyId, (String, String)>,
        binding_id: &str,
        hotkey_string: &str,
    ) -> Result<(), String> {
        let hotkey = parse_hotkey(hotkey_string)?;

        let id = manager
            .register(hotkey)
            .map_err(|e| format!("Failed to register handy-keys shortcut: {}", e))?;

        binding_to_hotkey.insert(binding_id.to_string(), id);
        hotkey_to_binding.insert(id, (binding_id.to_string(), hotkey_string.to_string()));

        debug!(
            "Registered handy-keys shortcut '{}' as '{}'",
            binding_id, hotkey_string
        );
        Ok(())
    }

    fn do_unregister(
        manager: &HotkeyManager,
        binding_to_hotkey: &mut HashMap<String, HotkeyId>,
        hotkey_to_binding: &mut HashMap<HotkeyId, (String, String)>,
        binding_id: &str,
    ) -> Result<(), String> {
        if let Some(id) = binding_to_hotkey.remove(binding_id) {
            manager
                .unregister(id)
                .map_err(|e| format!("Failed to unregister handy-keys shortcut: {}", e))?;
            hotkey_to_binding.remove(&id);
            debug!("Unregistered handy-keys shortcut '{}'", binding_id);
        }

        Ok(())
    }

    pub fn register(&self, binding: &ShortcutBinding) -> Result<(), String> {
        {
            let registered = self
                .registered_bindings
                .lock()
                .map_err(|_| "Failed to lock handy-keys registry".to_string())?;
            if registered.contains(&binding.id) {
                return Err(format!(
                    "Shortcut '{}' is already registered via handy-keys",
                    binding.id
                ));
            }
        }

        let (tx, rx) = mpsc::channel();
        self.command_sender
            .lock()
            .map_err(|_| "Failed to lock handy-keys command sender".to_string())?
            .send(ManagerCommand::Register {
                binding_id: binding.id.clone(),
                hotkey_string: binding.current_binding.clone(),
                response: tx,
            })
            .map_err(|_| "Failed to send handy-keys register command".to_string())?;

        rx.recv()
            .map_err(|_| "Failed to receive handy-keys register response".to_string())??;

        self.registered_bindings
            .lock()
            .map_err(|_| "Failed to lock handy-keys registry".to_string())?
            .insert(binding.id.clone());

        Ok(())
    }

    pub fn unregister(&self, binding: &ShortcutBinding) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.command_sender
            .lock()
            .map_err(|_| "Failed to lock handy-keys command sender".to_string())?
            .send(ManagerCommand::Unregister {
                binding_id: binding.id.clone(),
                response: tx,
            })
            .map_err(|_| "Failed to send handy-keys unregister command".to_string())?;

        rx.recv()
            .map_err(|_| "Failed to receive handy-keys unregister response".to_string())??;

        self.registered_bindings
            .lock()
            .map_err(|_| "Failed to lock handy-keys registry".to_string())?
            .remove(&binding.id);

        Ok(())
    }

    pub fn is_registered(&self, binding_id: &str) -> bool {
        self.registered_bindings
            .lock()
            .map(|registered| registered.contains(binding_id))
            .unwrap_or(false)
    }

    pub fn start_recording(&self, app: &AppHandle) -> Result<(), String> {
        if self.is_recording.load(Ordering::SeqCst) {
            return Err("HandyKeys recording is already active".to_string());
        }

        let listener = KeyboardListener::new()
            .map_err(|e| format!("Failed to create handy-keys listener: {}", e))?;

        {
            let mut recording_listener = self
                .recording_listener
                .lock()
                .map_err(|_| "Failed to lock handy-keys recording listener".to_string())?;
            *recording_listener = Some(listener);
        }

        self.is_recording.store(true, Ordering::SeqCst);
        self.recording_running.store(true, Ordering::SeqCst);

        let app_clone = app.clone();
        let recording_running = Arc::clone(&self.recording_running);
        thread::spawn(move || {
            Self::recording_loop(app_clone, recording_running);
        });

        Ok(())
    }

    fn recording_loop(app: AppHandle, running: Arc<AtomicBool>) {
        while running.load(Ordering::SeqCst) {
            let event = {
                let Some(state) = app.try_state::<HandyKeysState>() else {
                    break;
                };
                let listener = state.recording_listener.lock().ok();
                listener
                    .as_ref()
                    .and_then(|value| value.as_ref()?.try_recv())
            };

            if let Some(key_event) = event {
                let frontend_event = FrontendKeyEvent {
                    modifiers: modifiers_to_strings(key_event.modifiers),
                    key: key_event.key.map(|key| key.to_string().to_lowercase()),
                    is_key_down: key_event.is_key_down,
                    hotkey_string: key_event
                        .as_hotkey()
                        .map(|hotkey| hotkey.to_handy_string())
                        .unwrap_or_default(),
                };

                if let Err(err) = app.emit("handy-keys-event", &frontend_event) {
                    error!("Failed to emit handy-keys recording event: {}", err);
                }
            } else {
                thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }

    pub fn stop_recording(&self) -> Result<(), String> {
        self.is_recording.store(false, Ordering::SeqCst);
        self.recording_running.store(false, Ordering::SeqCst);

        let mut recording_listener = self
            .recording_listener
            .lock()
            .map_err(|_| "Failed to lock handy-keys recording listener".to_string())?;
        *recording_listener = None;

        Ok(())
    }
}

impl Drop for HandyKeysState {
    fn drop(&mut self) {
        self.recording_running.store(false, Ordering::SeqCst);
        self.is_recording.store(false, Ordering::SeqCst);

        if let Ok(sender) = self.command_sender.lock() {
            let _ = sender.send(ManagerCommand::Shutdown);
        }

        if let Ok(mut handle) = self.thread_handle.lock() {
            if let Some(handle) = handle.take() {
                let _ = handle.join();
            }
        }
    }
}

fn modifiers_to_strings(modifiers: handy_keys::Modifiers) -> Vec<String> {
    let mut result = Vec::new();

    if modifiers.contains(handy_keys::Modifiers::CTRL) {
        result.push("ctrl".to_string());
    }
    if modifiers.contains(handy_keys::Modifiers::OPT) {
        #[cfg(target_os = "macos")]
        result.push("option".to_string());
        #[cfg(not(target_os = "macos"))]
        result.push("alt".to_string());
    }
    if modifiers.contains(handy_keys::Modifiers::SHIFT) {
        result.push("shift".to_string());
    }
    if modifiers.contains(handy_keys::Modifiers::CMD) {
        #[cfg(target_os = "macos")]
        result.push("command".to_string());
        #[cfg(not(target_os = "macos"))]
        result.push("super".to_string());
    }
    if modifiers.contains(handy_keys::Modifiers::FN) {
        result.push("fn".to_string());
    }

    result
}

fn parse_hotkey(raw: &str) -> Result<Hotkey, String> {
    raw.parse::<Hotkey>()
        .or_else(|_| normalize_for_handy_keys(raw).parse::<Hotkey>())
        .map_err(|e| format!("Invalid shortcut for HandyKeys '{}': {}", raw, e))
}

fn normalize_for_handy_keys(raw: &str) -> String {
    raw.split('+')
        .map(|part| match part.trim().to_lowercase().as_str() {
            "win" | "windows" | "meta" | "cmd" | "command" => "super".to_string(),
            "option" => "alt".to_string(),
            "control" => "ctrl".to_string(),
            "esc" => "escape".to_string(),
            "return" => "enter".to_string(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

pub fn initialize(app: &AppHandle) -> Result<(), String> {
    if app.try_state::<HandyKeysState>().is_some() {
        return Ok(());
    }

    app.manage(HandyKeysState::new(app.clone())?);
    info!("HandyKeys backend initialized");
    Ok(())
}

pub fn validate_shortcut(raw: &str) -> Result<(), String> {
    if raw.trim().is_empty() {
        return Ok(());
    }

    parse_hotkey(raw).map(|_| ())
}

pub fn register_shortcut(app: &AppHandle, binding: ShortcutBinding) -> Result<(), String> {
    let state = app
        .try_state::<HandyKeysState>()
        .ok_or_else(|| "HandyKeys backend is not initialized".to_string())?;
    state.register(&binding)
}

pub fn unregister_shortcut(app: &AppHandle, binding: ShortcutBinding) -> Result<(), String> {
    let state = app
        .try_state::<HandyKeysState>()
        .ok_or_else(|| "HandyKeys backend is not initialized".to_string())?;
    state.unregister(&binding)
}

pub fn is_registered(app: &AppHandle, binding_id: &str) -> bool {
    app.try_state::<HandyKeysState>()
        .map(|state| state.is_registered(binding_id))
        .unwrap_or(false)
}

#[tauri::command]
#[specta::specta]
pub fn start_handy_keys_recording(app: AppHandle, binding_id: String) -> Result<(), String> {
    let settings = get_settings(&app);
    if settings.shortcut_engine != settings::ShortcutEngine::HandyKeys {
        return Err("HandyKeys is not the configured shortcut engine".to_string());
    }

    let _ = binding_id;
    let state = app
        .try_state::<HandyKeysState>()
        .ok_or_else(|| "HandyKeys backend is not initialized".to_string())?;
    state.start_recording(&app)
}

#[tauri::command]
#[specta::specta]
pub fn stop_handy_keys_recording(app: AppHandle) -> Result<(), String> {
    let settings = get_settings(&app);
    if settings.shortcut_engine != settings::ShortcutEngine::HandyKeys {
        return Err("HandyKeys is not the configured shortcut engine".to_string());
    }

    let state = app
        .try_state::<HandyKeysState>()
        .ok_or_else(|| "HandyKeys backend is not initialized".to_string())?;
    state.stop_recording()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_to_strings_returns_expected_platform_order() {
        let modifiers = handy_keys::Modifiers::CTRL
            | handy_keys::Modifiers::OPT
            | handy_keys::Modifiers::SHIFT
            | handy_keys::Modifiers::CMD
            | handy_keys::Modifiers::FN;

        #[cfg(target_os = "macos")]
        assert_eq!(
            modifiers_to_strings(modifiers),
            vec!["ctrl", "option", "shift", "command", "fn"]
        );

        #[cfg(not(target_os = "macos"))]
        assert_eq!(
            modifiers_to_strings(modifiers),
            vec!["ctrl", "alt", "shift", "super", "fn"]
        );
    }

    #[test]
    fn normalize_for_handy_keys_maps_common_modifier_aliases() {
        assert_eq!(
            normalize_for_handy_keys("Win+Option+Control+Esc+Return"),
            "super+alt+ctrl+escape+enter"
        );
    }

    #[test]
    fn normalize_for_handy_keys_trims_and_lowercases_unknown_parts() {
        assert_eq!(normalize_for_handy_keys(" Shift + A + F13 "), "shift+a+f13");
    }

    #[test]
    fn validate_shortcut_allows_blank_strings() {
        assert!(validate_shortcut("   ").is_ok());
    }

    #[test]
    fn validate_shortcut_accepts_aliases_after_normalization() {
        assert!(validate_shortcut("Control+Esc").is_ok());
    }
}
