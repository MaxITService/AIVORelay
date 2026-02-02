use log::{debug, error, info, warn};
use rdev::{Event, EventType, Key};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

/// State for tracking active key modifiers (Ctrl, Shift, Alt, Win)
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ModifierState {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
}

impl ModifierState {
    /// Update modifier state based on key event
    pub fn update(&mut self, key: Key, pressed: bool) {
        match key {
            Key::ControlLeft | Key::ControlRight => self.ctrl = pressed,
            Key::ShiftLeft | Key::ShiftRight => self.shift = pressed,
            Key::Alt | Key::AltGr => self.alt = pressed,
            Key::MetaLeft | Key::MetaRight => self.win = pressed,
            _ => {}
        }
    }

    /// Check if modifiers match the required state
    pub fn matches(&self, required: &ModifierState) -> bool {
        self.ctrl == required.ctrl
            && self.shift == required.shift
            && self.alt == required.alt
            && self.win == required.win
    }
}

/// A registered shortcut with its trigger key and required modifiers
/// For modifier-only shortcuts (like Ctrl+Alt), key will be None
#[derive(Debug, Clone)]
pub struct RegisteredShortcut {
    pub key: Option<Key>,
    pub modifiers: ModifierState,
    pub original_binding: String,
}

/// Shortcut event sent to the app
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShortcutEvent {
    pub id: String,
    pub binding: String,
    pub pressed: bool,
}

/// Main key listener manager with shortcut support
pub struct KeyListenerManager {
    app_handle: Arc<AppHandle>,
    running: Arc<Mutex<bool>>,
    modifiers: Arc<Mutex<ModifierState>>,
    shortcuts: Arc<Mutex<HashMap<String, RegisteredShortcut>>>,
    /// Track which shortcuts are currently "held down" to detect release
    active_shortcuts: Arc<Mutex<HashMap<String, bool>>>,
}

impl KeyListenerManager {
    /// Create a new key listener manager
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle: Arc::new(app_handle),
            running: Arc::new(Mutex::new(false)),
            modifiers: Arc::new(Mutex::new(ModifierState::default())),
            shortcuts: Arc::new(Mutex::new(HashMap::new())),
            active_shortcuts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a shortcut from a string like "ctrl+shift+a" or "caps lock"
    pub async fn register_shortcut(&self, id: String, binding: String) -> Result<(), String> {
        let (key, modifiers) = parse_shortcut_string(&binding)?;

        let shortcut = RegisteredShortcut {
            key,
            modifiers,
            original_binding: binding.clone(),
        };

        let mut shortcuts = self.shortcuts.lock().map_err(|e| e.to_string())?;
        shortcuts.insert(id.clone(), shortcut);
        info!("Registered rdev shortcut '{}': {}", id, binding);
        Ok(())
    }

    /// Unregister a shortcut by ID
    pub async fn unregister_shortcut(&self, id: &str) -> Result<(), String> {
        let mut shortcuts = self.shortcuts.lock().map_err(|e| e.to_string())?;
        if shortcuts.remove(id).is_some() {
            info!("Unregistered rdev shortcut '{}'", id);
            Ok(())
        } else {
            Err(format!("Shortcut '{}' not found", id))
        }
    }

    /// Check if a shortcut is registered
    pub async fn is_shortcut_registered(&self, id: &str) -> bool {
        let shortcuts = self.shortcuts.lock().unwrap_or_else(|e| e.into_inner());
        shortcuts.contains_key(id)
    }

    /// Start listening for keyboard events
    pub async fn start(&self) -> Result<(), String> {
        {
            let mut running_guard = self.running.lock().map_err(|e| e.to_string())?;
            if *running_guard {
                info!("Key listener already running");
                return Ok(());
            }
            *running_guard = true;
        }

        info!("Starting key listener");

        let app_handle = self.app_handle.clone();
        let running = self.running.clone();
        let modifiers = self.modifiers.clone();
        let shortcuts = self.shortcuts.clone();
        let active_shortcuts = self.active_shortcuts.clone();

        std::thread::spawn(move || {
            if let Err(e) = rdev::listen(move |event| {
                Self::handle_event(
                    event,
                    &app_handle,
                    &modifiers,
                    &shortcuts,
                    &active_shortcuts,
                );
            }) {
                error!("Failed to start key listener: {:?}", e);
                if let Ok(mut running_lock) = running.lock() {
                    *running_lock = false;
                }
            }
        });

        info!("Key listener started successfully");
        Ok(())
    }

    /// Stop listening for keyboard events
    pub async fn stop(&self) -> Result<(), String> {
        {
            let mut running = self.running.lock().map_err(|e| e.to_string())?;
            if !*running {
                info!("Key listener already stopped");
                return Ok(());
            }
            *running = false;
        }

        info!("Stopping key listener");

        if let Ok(mut modifiers) = self.modifiers.lock() {
            *modifiers = ModifierState::default();
        }

        if let Ok(mut active) = self.active_shortcuts.lock() {
            active.clear();
        }

        Ok(())
    }

    /// Handle individual keyboard events - must be non-blocking!
    fn handle_event(
        event: Event,
        app_handle: &Arc<AppHandle>,
        modifiers: &Arc<Mutex<ModifierState>>,
        shortcuts: &Arc<Mutex<HashMap<String, RegisteredShortcut>>>,
        active_shortcuts: &Arc<Mutex<HashMap<String, bool>>>,
    ) {
        match event.event_type {
            EventType::KeyPress(key) => {
                // Update modifiers - non-blocking with try_lock or unwrap_or_else
                let current_mods = {
                    let Ok(mut mods) = modifiers.try_lock() else {
                        return; // Skip if can't get lock immediately
                    };
                    mods.update(key, true);
                    mods.clone()
                };

                // Check if this key press matches any registered shortcut
                let Ok(shortcuts_guard) = shortcuts.try_lock() else {
                    return;
                };
                let Ok(mut active_guard) = active_shortcuts.try_lock() else {
                    return;
                };

                for (id, shortcut) in shortcuts_guard.iter() {
                    let matches = match shortcut.key {
                        // Regular shortcut with main key
                        Some(shortcut_key) => {
                            shortcut_key == key && current_mods.matches(&shortcut.modifiers)
                        }
                        // Modifier-only shortcut - fire when modifiers match exactly
                        None => {
                            current_mods.matches(&shortcut.modifiers)
                                && Self::is_modifier_key(key)
                        }
                    };

                    if matches {
                        // Only fire if not already active (prevent key repeat)
                        if !active_guard.get(id).copied().unwrap_or(false) {
                            active_guard.insert(id.clone(), true);
                            debug!("Shortcut pressed: {} ({})", id, shortcut.original_binding);

                            let event = ShortcutEvent {
                                id: id.clone(),
                                binding: shortcut.original_binding.clone(),
                                pressed: true,
                            };
                            if let Err(e) = app_handle.emit("rdev-shortcut", &event) {
                                warn!("Failed to emit rdev-shortcut event: {}", e);
                            }
                        }
                    }
                }
            }
            EventType::KeyRelease(key) => {
                // Update modifiers
                let current_mods = {
                    let Ok(mut mods) = modifiers.try_lock() else {
                        return;
                    };
                    mods.update(key, false);
                    mods.clone()
                };

                // Check if releasing this key deactivates any shortcuts
                let Ok(shortcuts_guard) = shortcuts.try_lock() else {
                    return;
                };
                let Ok(mut active_guard) = active_shortcuts.try_lock() else {
                    return;
                };

                for (id, shortcut) in shortcuts_guard.iter() {
                    let should_release = match shortcut.key {
                        // Release if main key is released
                        Some(shortcut_key) => shortcut_key == key,
                        // For modifier-only: release if any required modifier is released
                        None => !current_mods.matches(&shortcut.modifiers),
                    };

                    // Also release if a required modifier is released (for regular shortcuts too)
                    let modifier_released = !current_mods.matches(&shortcut.modifiers);

                    if should_release || modifier_released {
                        if active_guard.get(id).copied().unwrap_or(false) {
                            active_guard.insert(id.clone(), false);
                            debug!("Shortcut released: {} ({})", id, shortcut.original_binding);

                            let event = ShortcutEvent {
                                id: id.clone(),
                                binding: shortcut.original_binding.clone(),
                                pressed: false,
                            };
                            if let Err(e) = app_handle.emit("rdev-shortcut", &event) {
                                warn!("Failed to emit rdev-shortcut event: {}", e);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Check if a key is a modifier key
    fn is_modifier_key(key: Key) -> bool {
        matches!(
            key,
            Key::ControlLeft
                | Key::ControlRight
                | Key::ShiftLeft
                | Key::ShiftRight
                | Key::Alt
                | Key::AltGr
                | Key::MetaLeft
                | Key::MetaRight
        )
    }

    /// Check if listener is running
    pub async fn is_running(&self) -> bool {
        self.running
            .lock()
            .map(|g| *g)
            .unwrap_or(false)
    }

    /// Get current modifier state
    pub async fn get_modifier_state(&self) -> ModifierState {
        self.modifiers
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    /// Get list of registered shortcut IDs
    pub async fn get_registered_shortcuts(&self) -> Vec<String> {
        self.shortcuts
            .lock()
            .map(|g| g.keys().cloned().collect())
            .unwrap_or_default()
    }
}

/// Parse a shortcut string like "ctrl+shift+a", "caps lock", or "ctrl+alt" into key and modifiers
/// Returns (Option<Key>, ModifierState) - key is None for modifier-only shortcuts
pub fn parse_shortcut_string(binding: &str) -> Result<(Option<Key>, ModifierState), String> {
    let binding = binding.to_lowercase().trim().to_string();
    let parts: Vec<&str> = binding.split('+').map(|s| s.trim()).collect();

    let mut modifiers = ModifierState::default();
    let mut main_key: Option<Key> = None;

    for part in parts {
        match part {
            "ctrl" | "control" => modifiers.ctrl = true,
            "shift" => modifiers.shift = true,
            "alt" => modifiers.alt = true,
            "win" | "super" | "meta" | "cmd" | "command" => modifiers.win = true,
            key_str => {
                if main_key.is_some() {
                    return Err(format!(
                        "Multiple main keys in shortcut: already have a key, found '{}'",
                        key_str
                    ));
                }
                main_key = Some(string_to_rdev_key(key_str)?);
            }
        }
    }

    // Modifier-only shortcuts are valid (e.g., Ctrl+Alt)
    // But we need at least one modifier if there's no main key
    if main_key.is_none()
        && !modifiers.ctrl
        && !modifiers.shift
        && !modifiers.alt
        && !modifiers.win
    {
        return Err("Shortcut must have at least one key or modifier".to_string());
    }

    Ok((main_key, modifiers))
}

/// Convert a string to an rdev::Key
fn string_to_rdev_key(s: &str) -> Result<Key, String> {
    let s = s.to_lowercase();
    let s = s.trim();

    match s {
        // Caps Lock - the main reason for this implementation!
        "caps lock" | "capslock" | "caps" => Ok(Key::CapsLock),

        // Function keys F1-F24
        "f1" => Ok(Key::F1),
        "f2" => Ok(Key::F2),
        "f3" => Ok(Key::F3),
        "f4" => Ok(Key::F4),
        "f5" => Ok(Key::F5),
        "f6" => Ok(Key::F6),
        "f7" => Ok(Key::F7),
        "f8" => Ok(Key::F8),
        "f9" => Ok(Key::F9),
        "f10" => Ok(Key::F10),
        "f11" => Ok(Key::F11),
        "f12" => Ok(Key::F12),
        "f13" => Ok(Key::F13),
        "f14" => Ok(Key::F14),
        "f15" => Ok(Key::F15),
        "f16" => Ok(Key::F16),
        "f17" => Ok(Key::F17),
        "f18" => Ok(Key::F18),
        "f19" => Ok(Key::F19),
        "f20" => Ok(Key::F20),
        "f21" => Ok(Key::F21),
        "f22" => Ok(Key::F22),
        "f23" => Ok(Key::F23),
        "f24" => Ok(Key::F24),

        // Special keys
        "space" | "spacebar" => Ok(Key::Space),
        "enter" | "return" => Ok(Key::Return),
        "tab" => Ok(Key::Tab),
        "backspace" | "back" => Ok(Key::Backspace),
        "escape" | "esc" => Ok(Key::Escape),
        "delete" | "del" => Ok(Key::Delete),
        "insert" | "ins" => Ok(Key::Insert),
        "home" => Ok(Key::Home),
        "end" => Ok(Key::End),
        "pageup" | "page up" | "pgup" => Ok(Key::PageUp),
        "pagedown" | "page down" | "pgdn" => Ok(Key::PageDown),

        // Arrow keys
        "up" | "arrowup" => Ok(Key::UpArrow),
        "down" | "arrowdown" => Ok(Key::DownArrow),
        "left" | "arrowleft" => Ok(Key::LeftArrow),
        "right" | "arrowright" => Ok(Key::RightArrow),

        // Numpad
        "num0" | "numpad0" => Ok(Key::Kp0),
        "num1" | "numpad1" => Ok(Key::Kp1),
        "num2" | "numpad2" => Ok(Key::Kp2),
        "num3" | "numpad3" => Ok(Key::Kp3),
        "num4" | "numpad4" => Ok(Key::Kp4),
        "num5" | "numpad5" => Ok(Key::Kp5),
        "num6" | "numpad6" => Ok(Key::Kp6),
        "num7" | "numpad7" => Ok(Key::Kp7),
        "num8" | "numpad8" => Ok(Key::Kp8),
        "num9" | "numpad9" => Ok(Key::Kp9),
        "nummultiply" | "numpad*" | "num*" => Ok(Key::KpMultiply),
        "numadd" | "numpad+" | "num+" => Ok(Key::KpPlus),
        "numsubtract" | "numpad-" | "num-" => Ok(Key::KpMinus),
        "numdecimal" | "numpad." | "num." => Ok(Key::KpDecimal),
        "numdivide" | "numpad/" | "num/" => Ok(Key::KpDivide),
        "numenter" => Ok(Key::KpReturn),

        // Letters
        "a" => Ok(Key::KeyA),
        "b" => Ok(Key::KeyB),
        "c" => Ok(Key::KeyC),
        "d" => Ok(Key::KeyD),
        "e" => Ok(Key::KeyE),
        "f" => Ok(Key::KeyF),
        "g" => Ok(Key::KeyG),
        "h" => Ok(Key::KeyH),
        "i" => Ok(Key::KeyI),
        "j" => Ok(Key::KeyJ),
        "k" => Ok(Key::KeyK),
        "l" => Ok(Key::KeyL),
        "m" => Ok(Key::KeyM),
        "n" => Ok(Key::KeyN),
        "o" => Ok(Key::KeyO),
        "p" => Ok(Key::KeyP),
        "q" => Ok(Key::KeyQ),
        "r" => Ok(Key::KeyR),
        "s" => Ok(Key::KeyS),
        "t" => Ok(Key::KeyT),
        "u" => Ok(Key::KeyU),
        "v" => Ok(Key::KeyV),
        "w" => Ok(Key::KeyW),
        "x" => Ok(Key::KeyX),
        "y" => Ok(Key::KeyY),
        "z" => Ok(Key::KeyZ),

        // Numbers
        "0" => Ok(Key::Num0),
        "1" => Ok(Key::Num1),
        "2" => Ok(Key::Num2),
        "3" => Ok(Key::Num3),
        "4" => Ok(Key::Num4),
        "5" => Ok(Key::Num5),
        "6" => Ok(Key::Num6),
        "7" => Ok(Key::Num7),
        "8" => Ok(Key::Num8),
        "9" => Ok(Key::Num9),

        // Punctuation
        "`" | "backquote" | "grave" => Ok(Key::BackQuote),
        "-" | "minus" => Ok(Key::Minus),
        "=" | "equal" | "equals" => Ok(Key::Equal),
        "[" | "bracketleft" => Ok(Key::LeftBracket),
        "]" | "bracketright" => Ok(Key::RightBracket),
        "\\" | "backslash" => Ok(Key::BackSlash),
        ";" | "semicolon" => Ok(Key::SemiColon),
        "'" | "quote" | "apostrophe" => Ok(Key::Quote),
        "," | "comma" => Ok(Key::Comma),
        "." | "period" => Ok(Key::Dot),
        "/" | "slash" => Ok(Key::Slash),

        // Print Screen, Scroll Lock, Pause
        "printscreen" | "print" | "prtsc" => Ok(Key::PrintScreen),
        "scrolllock" | "scroll" => Ok(Key::ScrollLock),
        "pause" | "break" => Ok(Key::Pause),

        // Numlock
        "numlock" => Ok(Key::NumLock),

        // Numpad delete (maps to Delete - KpDelete not available in rdev)
        "kpdelete" | "numpaddelete" | "numdel" => Ok(Key::Delete),

        // International backslash (non-US keyboards, key between left shift and Z)
        "intlbackslash" | "oem102" => Ok(Key::IntlBackslash),

        _ => Err(format!("Unknown key: '{}'", s)),
    }
}

/// Tauri state wrapper for KeyListenerManager
pub struct KeyListenerState {
    pub manager: Arc<KeyListenerManager>,
}

impl KeyListenerState {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            manager: Arc::new(KeyListenerManager::new(app_handle)),
        }
    }
}
