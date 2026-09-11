use log::{debug, error, info, warn};
use rdev::{EventType, Key};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

const DECAPITALIZE_MONITOR_SHORTCUT_ID_PREFIX: &str = "__text_replacement_decapitalize_monitor__";

/// State for tracking active key modifiers (Ctrl, Shift, Alt, Win)
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ModifierState {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
    #[serde(skip)]
    held_sides: u8,
}

impl ModifierState {
    /// Update modifier state based on key event
    pub fn update(&mut self, key: Key, pressed: bool) {
        let bit = match key {
            Key::ControlLeft => 1,
            Key::ControlRight => 2,
            Key::ShiftLeft => 4,
            Key::ShiftRight => 8,
            Key::Alt => 16,
            Key::AltGr => 32,
            Key::MetaLeft => 64,
            Key::MetaRight => 128,
            _ => return,
        };
        if pressed {
            self.held_sides |= bit;
        } else {
            self.held_sides &= !bit;
        }
        self.ctrl = self.held_sides & 3 != 0;
        self.shift = self.held_sides & 12 != 0;
        self.alt = self.held_sides & 48 != 0;
        self.win = self.held_sides & 192 != 0;
    }

    /// Check if modifiers match the required state
    pub fn matches(&self, required: &ModifierState) -> bool {
        self.ctrl == required.ctrl
            && self.shift == required.shift
            && self.alt == required.alt
            && self.win == required.win
    }

    /// Check if all required modifiers are currently pressed.
    pub fn contains_required(&self, required: &ModifierState) -> bool {
        (!required.ctrl || self.ctrl)
            && (!required.shift || self.shift)
            && (!required.alt || self.alt)
            && (!required.win || self.win)
    }
}

/// A registered shortcut with its trigger key and required modifiers
/// For modifier-only shortcuts (like Ctrl+Alt), key will be None
#[derive(Debug, Clone)]
pub struct RegisteredShortcut {
    pub key: Option<Key>,
    pub modifiers: ModifierState,
    pub original_binding: String,
    /// When true, a shortcut with a main key matches regardless of extra modifiers.
    /// Used by passive monitor features that should trigger on key presence in combos.
    pub match_main_key_in_any_combo: bool,
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
    event_generation: Arc<AtomicU64>,
    listener_thread_started: Arc<AtomicBool>,
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
            event_generation: Arc::new(AtomicU64::new(0)),
            listener_thread_started: Arc::new(AtomicBool::new(false)),
            modifiers: Arc::new(Mutex::new(ModifierState::default())),
            shortcuts: Arc::new(Mutex::new(HashMap::new())),
            active_shortcuts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a shortcut from a string like "ctrl+shift+a" or "caps lock"
    pub async fn register_shortcut(&self, id: String, binding: String) -> Result<(), String> {
        let (key, modifiers) = parse_shortcut_string(&binding)?;
        let match_main_key_in_any_combo = id.starts_with(DECAPITALIZE_MONITOR_SHORTCUT_ID_PREFIX);

        let shortcut = RegisteredShortcut {
            key,
            modifiers,
            original_binding: binding.clone(),
            match_main_key_in_any_combo,
        };

        let mut shortcuts = self.shortcuts.lock().map_err(|e| e.to_string())?;
        if !match_main_key_in_any_combo {
            if let Some((existing_id, existing)) =
                shortcuts.iter().find(|(existing_id, existing)| {
                    existing_id.as_str() != id.as_str()
                        && !existing.match_main_key_in_any_combo
                        && same_physical_shortcut(existing, &shortcut)
                })
            {
                return Err(format!(
                    "Shortcut '{}' is already in use by '{}' ({})",
                    binding, existing_id, existing.original_binding
                ));
            }
        }
        // A previous binding for this ID may have been removed while held;
        // its release is no longer routed through the shortcut map.
        self.active_shortcuts.lock().map_err(|e| e.to_string())?.remove(&id);
        shortcuts.insert(id.clone(), shortcut);
        info!("Registered rdev shortcut '{}': {}", id, binding);
        Ok(())
    }

    /// Unregister a shortcut by ID
    pub async fn unregister_shortcut(&self, id: &str) -> Result<(), String> {
        let mut shortcuts = self.shortcuts.lock().map_err(|e| e.to_string())?;
        self.active_shortcuts.lock().map_err(|e| e.to_string())?.remove(id);
        if shortcuts.remove(id).is_some() {
            info!("Unregistered rdev shortcut '{}'", id);
            Ok(())
        } else {
            Err(format!("Shortcut '{}' not found", id))
        }
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
            self.event_generation.fetch_add(1, Ordering::SeqCst);
        }

        if self.listener_thread_started.load(Ordering::SeqCst) {
            info!("Key listener thread already initialized; event processing re-enabled");
            return Ok(());
        }

        if self
            .listener_thread_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            info!("Key listener thread already initialized; event processing re-enabled");
            return Ok(());
        }

        info!("Starting key listener thread");

        let app_handle = self.app_handle.clone();
        let running = self.running.clone();
        let modifiers = self.modifiers.clone();
        let shortcuts = self.shortcuts.clone();
        let active_shortcuts = self.active_shortcuts.clone();
        let listener_thread_started = self.listener_thread_started.clone();
        let event_generation = self.event_generation.clone();

        std::thread::spawn(move || {
            let running_for_events = running.clone();
            let generation_for_events = event_generation.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            // The OS hook only enqueues key transitions. Waiting for a
            // registration lock must not drop KeyRelease or block the hook.
            std::thread::spawn(move || {
                while let Ok((generation, event_type)) = receiver.recv() {
                    Self::handle_event(
                        event_type,
                        generation,
                        &generation_for_events,
                        &app_handle,
                        &running_for_events,
                        &modifiers,
                        &shortcuts,
                        &active_shortcuts,
                    );
                }
            });
            if let Err(e) = rdev::listen(move |event| {
                if matches!(event.event_type, EventType::KeyPress(_) | EventType::KeyRelease(_)) {
                    let generation = event_generation.load(Ordering::SeqCst);
                    let _ = sender.send((generation, event.event_type));
                }
            }) {
                error!("Failed to start key listener: {:?}", e);
                if let Ok(mut running_lock) = running.lock() {
                    *running_lock = false;
                }
                listener_thread_started.store(false, Ordering::SeqCst);
            }
        });

        info!("Key listener enabled");
        Ok(())
    }

    /// Stop listening for keyboard events
    pub async fn stop(&self) -> Result<(), String> {
        let mut running = self.running.lock().map_err(|e| e.to_string())?;
        if !*running {
            info!("Key listener already stopped");
            return Ok(());
        }
        *running = false;
        self.event_generation.fetch_add(1, Ordering::SeqCst);

        info!("Stopping key listener");

        if let Ok(mut modifiers) = self.modifiers.lock() {
            *modifiers = ModifierState::default();
        }

        if let Ok(mut active) = self.active_shortcuts.lock() {
            active.clear();
        }

        Ok(())
    }

    /// Process queued transitions off the OS hook, in arrival order.
    fn handle_event(
        event_type: EventType,
        generation: u64,
        event_generation: &AtomicU64,
        app_handle: &Arc<AppHandle>,
        running: &Arc<Mutex<bool>>,
        modifiers: &Arc<Mutex<ModifierState>>,
        shortcuts: &Arc<Mutex<HashMap<String, RegisteredShortcut>>>,
        active_shortcuts: &Arc<Mutex<HashMap<String, bool>>>,
    ) {
        let Ok(running_guard) = running.lock() else {
            return;
        };
        if !*running_guard || event_generation.load(Ordering::SeqCst) != generation {
            return;
        }
        let mut emitted = Vec::new();

        match event_type {
            EventType::KeyPress(key) => {
                // The worker can wait for state without losing transitions.
                let current_mods = {
                    let Ok(mut mods) = modifiers.lock() else {
                        return;
                    };
                    mods.update(key, true);
                    mods.clone()
                };

                // Check if this key press matches any registered shortcut
                let Ok(shortcuts_guard) = shortcuts.lock() else {
                    return;
                };
                let Ok(mut active_guard) = active_shortcuts.lock() else {
                    return;
                };

                for (id, shortcut) in shortcuts_guard.iter() {
                    let matches = match shortcut.key {
                        // Regular shortcut with main key
                        Some(shortcut_key) => {
                            shortcut_key == key
                                && if shortcut.match_main_key_in_any_combo {
                                    true
                                } else {
                                    current_mods.matches(&shortcut.modifiers)
                                }
                        }
                        // Modifier-only shortcut
                        None => {
                            Self::is_modifier_key(key)
                                && if shortcut.match_main_key_in_any_combo {
                                    current_mods.contains_required(&shortcut.modifiers)
                                } else {
                                    current_mods.matches(&shortcut.modifiers)
                                }
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
                            emitted.push(event);
                        }
                    }
                }
            }
            EventType::KeyRelease(key) => {
                // Update modifiers
                let current_mods = {
                    let Ok(mut mods) = modifiers.lock() else {
                        return;
                    };
                    mods.update(key, false);
                    mods.clone()
                };

                // Check if releasing this key deactivates any shortcuts
                let Ok(shortcuts_guard) = shortcuts.lock() else {
                    return;
                };
                let Ok(mut active_guard) = active_shortcuts.lock() else {
                    return;
                };

                for (id, shortcut) in shortcuts_guard.iter() {
                    let should_release = match shortcut.key {
                        // Release if main key is released
                        Some(shortcut_key) => shortcut_key == key,
                        // For modifier-only: release if any required modifier is released
                        None => {
                            if shortcut.match_main_key_in_any_combo {
                                !current_mods.contains_required(&shortcut.modifiers)
                            } else {
                                !current_mods.matches(&shortcut.modifiers)
                            }
                        }
                    };

                    // Also release if a required modifier is released (for regular shortcuts too)
                    let modifier_released = if shortcut.match_main_key_in_any_combo {
                        false
                    } else {
                        !current_mods.matches(&shortcut.modifiers)
                    };

                    if should_release || modifier_released {
                        if active_guard.get(id).copied().unwrap_or(false) {
                            active_guard.insert(id.clone(), false);
                            debug!("Shortcut released: {} ({})", id, shortcut.original_binding);

                            let event = ShortcutEvent {
                                id: id.clone(),
                                binding: shortcut.original_binding.clone(),
                                pressed: false,
                            };
                            emitted.push(event);
                        }
                    }
                }
            }
            _ => {}
        }
        // Rust event subscribers can register/unregister shortcuts. Never
        // invoke them while holding listener-state locks.
        drop(running_guard);
        for event in emitted {
            if event_generation.load(Ordering::SeqCst) != generation {
                break;
            }
            if let Err(e) = app_handle.emit("rdev-shortcut", &event) {
                warn!("Failed to emit rdev-shortcut event: {}", e);
            }
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
}

fn same_physical_shortcut(left: &RegisteredShortcut, right: &RegisteredShortcut) -> bool {
    left.key == right.key
        && left.modifiers.ctrl == right.modifiers.ctrl
        && left.modifiers.shift == right.modifiers.shift
        && left.modifiers.alt == right.modifiers.alt
        && left.modifiers.win == right.modifiers.win
}

/// Parse a shortcut string like "ctrl+shift+a", "caps lock", or "ctrl+alt" into key and modifiers
/// Returns (Option<Key>, ModifierState) - key is None for modifier-only shortcuts
pub fn parse_shortcut_string(binding: &str) -> Result<(Option<Key>, ModifierState), String> {
    let normalized = normalize_shortcut_binding(binding);
    let parts: Vec<&str> = normalized.split('+').map(|s| s.trim()).collect();

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
    if main_key.is_none() && !modifiers.ctrl && !modifiers.shift && !modifiers.alt && !modifiers.win
    {
        return Err("Shortcut must have at least one key or modifier".to_string());
    }

    Ok((main_key, modifiers))
}

fn normalize_shortcut_binding(raw: &str) -> String {
    let mut normalized = raw.trim().to_lowercase();
    // Legacy frontend token used "numpad +" which collides with '+' as the delimiter.
    normalized = normalized.replace("numpad +", "numadd");
    normalized.replace("numpad+", "numadd")
}

/// Convert a string to an rdev::Key
fn string_to_rdev_key(s: &str) -> Result<Key, String> {
    let s: String = s.chars().filter(|ch| !ch.is_whitespace()).collect::<String>().to_lowercase();
    let s = s.as_str();

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

#[cfg(test)]
mod tests {
    use super::*;

    fn modifiers(ctrl: bool, shift: bool, alt: bool, win: bool) -> ModifierState {
        ModifierState {
            ctrl,
            shift,
            alt,
            win,
            ..ModifierState::default()
        }
    }

    #[test]
    fn modifier_state_tracks_both_sides_and_ignores_regular_keys() {
        let mut state = ModifierState::default();
        state.update(Key::ControlRight, true);
        state.update(Key::ShiftLeft, true);
        state.update(Key::AltGr, true);
        state.update(Key::MetaRight, true);
        state.update(Key::KeyA, false);
        assert!(state.matches(&modifiers(true, true, true, true)));

        state.update(Key::ControlLeft, false);
        state.update(Key::ShiftRight, false);
        state.update(Key::Alt, false);
        state.update(Key::MetaLeft, false);
        assert!(state.matches(&modifiers(true, true, true, true)));
        state.update(Key::ControlRight, false);
        state.update(Key::ShiftLeft, false);
        state.update(Key::AltGr, false);
        state.update(Key::MetaRight, false);
        assert!(state.matches(&ModifierState::default()));
    }

    #[test]
    fn releasing_one_side_preserves_the_other_and_repeat_is_idempotent() {
        for (left, right) in [
            (Key::ControlLeft, Key::ControlRight),
            (Key::ShiftLeft, Key::ShiftRight),
            (Key::Alt, Key::AltGr),
            (Key::MetaLeft, Key::MetaRight),
        ] {
            let mut state = ModifierState::default();
            state.update(left, true);
            state.update(right, true);
            state.update(right, true);
            state.update(left, false);
            assert!(!state.matches(&ModifierState::default()));
            state.update(right, false);
            assert!(state.matches(&ModifierState::default()));
        }
    }

    #[test]
    fn parser_accepts_spaced_frontend_key_names() {
        for (name, expected) in [
            ("numpad 1", Key::Kp1),
            ("print screen", Key::PrintScreen),
            ("scroll lock", Key::ScrollLock),
            ("num lock", Key::NumLock),
            ("numpad /", Key::KpDivide),
            ("numpad -", Key::KpMinus),
        ] {
            assert_eq!(parse_shortcut_string(&format!("ctrl+{name}")).unwrap().0, Some(expected));
        }
    }

    #[test]
    fn exact_and_subset_modifier_matching_have_distinct_semantics() {
        let pressed = modifiers(true, true, true, false);
        let required = modifiers(true, false, true, false);

        assert!(!pressed.matches(&required));
        assert!(pressed.contains_required(&required));
        assert!(!required.contains_required(&pressed));
    }

    #[test]
    fn shortcut_parser_normalizes_case_whitespace_and_modifier_aliases() {
        let (key, state) = parse_shortcut_string("  Control + SHIFT + Command + A ").unwrap();

        assert_eq!(key, Some(Key::KeyA));
        assert!(state.matches(&modifiers(true, true, false, true)));
    }

    #[test]
    fn shortcut_parser_accepts_modifier_only_bindings() {
        let (key, state) = parse_shortcut_string("ctrl+alt").unwrap();

        assert_eq!(key, None);
        assert!(state.matches(&modifiers(true, false, true, false)));
    }

    #[test]
    fn shortcut_parser_preserves_legacy_numpad_plus_bindings() {
        for binding in ["numpad +", "numpad+", "ctrl+numpad +", "ctrl+numpad+"] {
            let (key, state) = parse_shortcut_string(binding).unwrap();
            assert_eq!(key, Some(Key::KpPlus), "{binding}");
            assert_eq!(state.ctrl, binding.starts_with("ctrl"), "{binding}");
        }
    }

    #[test]
    fn shortcut_parser_covers_extended_keys_and_rejects_unknown_ones() {
        let cases = [
            ("caps", Key::CapsLock),
            ("f24", Key::F24),
            ("page down", Key::PageDown),
            ("oem102", Key::IntlBackslash),
            ("numdel", Key::Delete),
        ];
        for (binding, expected) in cases {
            assert_eq!(parse_shortcut_string(binding).unwrap().0, Some(expected));
        }

        assert!(parse_shortcut_string("").is_err());
        assert!(parse_shortcut_string("hyper").is_err());
    }

    #[test]
    fn shortcut_parser_rejects_multiple_main_keys_with_context() {
        let error = parse_shortcut_string("ctrl+a+b").unwrap_err();
        assert!(error.contains("Multiple main keys"));
        assert!(error.contains("'b'"));
    }

    #[test]
    fn physical_shortcut_identity_ignores_display_metadata_only() {
        let first = RegisteredShortcut {
            key: Some(Key::KeyK),
            modifiers: modifiers(true, true, false, false),
            original_binding: "ctrl+shift+k".to_string(),
            match_main_key_in_any_combo: false,
        };
        let mut same = first.clone();
        same.original_binding = "CONTROL + SHIFT + K".to_string();
        same.match_main_key_in_any_combo = true;
        assert!(same_physical_shortcut(&first, &same));

        let mut different = same;
        different.modifiers.shift = false;
        assert!(!same_physical_shortcut(&first, &different));
    }
}
