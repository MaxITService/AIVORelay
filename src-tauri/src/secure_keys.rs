//! API key storage.
//!
//! On Windows, API keys are stored in the OS credential vault (Credential Manager).
//! On Linux and macOS they are stored in the app's settings store file, under a
//! dedicated entry that is separate from the serialized settings, matching upstream
//! Handy's plain settings-file storage.

use anyhow::{anyhow, Result};
#[cfg(target_os = "windows")]
use log::{debug, warn};

const SERVICE_NAME: &str = "fi.maxits.aivorelay";

/// Key type prefix for credential storage
#[derive(Debug, Clone, Copy)]
pub enum KeyType {
    /// Post-processing LLM API key (per provider)
    PostProcess,
    /// AI Replace LLM API key (per provider)
    AiReplace,
    /// Voice Command LLM API key (per provider)
    VoiceCommand,
    /// Soniox cloud STT API key
    SonioxStt,
    /// Deepgram cloud STT API key
    DeepgramStt,
    /// Text-to-Speech API key stored separately per provider
    Tts,
    /// TTS LLM preprocessing API key stored separately per provider
    TtsLlm,
}

impl KeyType {
    fn prefix(&self) -> &'static str {
        match self {
            KeyType::PostProcess => "post_process_api_key",
            KeyType::AiReplace => "ai_replace_api_key",
            KeyType::VoiceCommand => "voice_command_api_key",
            KeyType::SonioxStt => "soniox_api_key",
            KeyType::DeepgramStt => "deepgram_api_key",
            KeyType::Tts => "tts_api_key",
            KeyType::TtsLlm => "tts_llm_api_key",
        }
    }

    /// Build the credential user/account name
    fn credential_name(&self, provider_id: Option<&str>) -> String {
        match provider_id {
            Some(id) => format!("{}_{}", self.prefix(), id),
            None => self.prefix().to_string(),
        }
    }
}

// ============================================================================
// Windows implementation using keyring crate
// ============================================================================

#[cfg(target_os = "windows")]
pub fn set_api_key(key_type: KeyType, provider_id: Option<&str>, key: &str) -> Result<()> {
    let credential_name = key_type.credential_name(provider_id);
    debug!("Storing API key in credential manager: {}", credential_name);

    let entry = keyring::Entry::new(SERVICE_NAME, &credential_name)?;

    if key.trim().is_empty() {
        // If key is empty, delete the credential instead of storing empty string
        match entry.delete_password() {
            Ok(()) => {
                debug!("Deleted empty credential: {}", credential_name);
                Ok(())
            }
            Err(keyring::Error::NoEntry) => {
                // Already doesn't exist, that's fine
                Ok(())
            }
            Err(e) => Err(anyhow!("Failed to delete credential: {}", e)),
        }
    } else {
        entry
            .set_password(key)
            .map_err(|e| anyhow!("Failed to store API key: {}", e))
    }
}

#[cfg(target_os = "windows")]
pub fn get_api_key(key_type: KeyType, provider_id: Option<&str>) -> Result<String> {
    let credential_name = key_type.credential_name(provider_id);

    let entry = keyring::Entry::new(SERVICE_NAME, &credential_name)?;
    match entry.get_password() {
        Ok(key) => Ok(key),
        Err(keyring::Error::NoEntry) => {
            // No credential stored - return empty string (not an error)
            Ok(String::new())
        }
        Err(e) => Err(anyhow!("Failed to read API key: {}", e)),
    }
}

// ============================================================================
// Non-Windows implementation using the settings store file
// ============================================================================

#[cfg(not(target_os = "windows"))]
static APP_HANDLE: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

/// Registers the app handle used by the non-Windows key store. No-op on Windows.
pub fn init(app: &tauri::AppHandle) {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = APP_HANDLE.set(app.clone());
        // A store written by an earlier version may still be readable by others.
        crate::settings::restrict_settings_store_permissions(app);
    }
    #[cfg(target_os = "windows")]
    {
        let _ = app;
    }
}

#[cfg(not(target_os = "windows"))]
const STORE_ENTRY: &str = "provider_api_keys";

#[cfg(not(target_os = "windows"))]
fn key_store() -> Result<std::sync::Arc<tauri_plugin_store::Store<tauri::Wry>>> {
    use tauri_plugin_store::StoreExt;

    let app = APP_HANDLE
        .get()
        .ok_or_else(|| anyhow!("API key storage is not initialized"))?;
    app.store(crate::portable::store_path(
        crate::settings::SETTINGS_STORE_PATH,
    ))
    .map_err(|e| anyhow!("Failed to open API key store: {}", e))
}

/// Serializes read-modify-write cycles so concurrent saves cannot lose a key.
#[cfg(not(target_os = "windows"))]
static KEY_STORE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The stored keys. An entry of the wrong shape is an error, not an empty map:
/// treating it as empty would let the next save silently wipe every key.
#[cfg(not(target_os = "windows"))]
fn read_key_map(
    store: &tauri_plugin_store::Store<tauri::Wry>,
) -> Result<std::collections::HashMap<String, String>> {
    match store.get(STORE_ENTRY) {
        None => Ok(Default::default()),
        Some(value) => serde_json::from_value(value)
            .map_err(|e| anyhow!("Stored API keys are unreadable: {}", e)),
    }
}

#[cfg(not(target_os = "windows"))]
pub fn set_api_key(key_type: KeyType, provider_id: Option<&str>, key: &str) -> Result<()> {
    let credential_name = key_type.credential_name(provider_id);
    let _guard = KEY_STORE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let store = key_store()?;
    let previous = store.get(STORE_ENTRY);
    let mut keys = read_key_map(&store)?;

    if key.trim().is_empty() {
        keys.remove(&credential_name);
    } else {
        keys.insert(credential_name, key.to_string());
    }

    store.set(STORE_ENTRY, serde_json::to_value(&keys)?);
    let app = APP_HANDLE
        .get()
        .ok_or_else(|| anyhow!("API key storage is not initialized"))?;
    if let Err(error) = crate::settings::save_store_privately(app, &store) {
        // Keep memory and disk in agreement: a key that was not saved must not
        // look saved until the next restart.
        match previous {
            Some(value) => store.set(STORE_ENTRY, value),
            None => {
                store.delete(STORE_ENTRY);
            }
        }
        return Err(anyhow!("Failed to store API key: {}", error));
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn get_api_key(key_type: KeyType, provider_id: Option<&str>) -> Result<String> {
    let credential_name = key_type.credential_name(provider_id);
    let _guard = KEY_STORE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let store = key_store()?;
    Ok(read_key_map(&store)?
        .remove(&credential_name)
        .unwrap_or_default())
}

// ============================================================================
// Convenience functions for specific key types
// ============================================================================

/// Get a post-processing API key for a specific provider
pub fn get_post_process_api_key(provider_id: &str) -> String {
    // Outside Windows these keys live in the settings JSON, as before.
    #[cfg(not(target_os = "windows"))]
    if let Some(app) = APP_HANDLE.get() {
        return crate::settings::get_settings(app)
            .post_process_api_keys
            .get(provider_id)
            .cloned()
            .unwrap_or_default();
    }
    get_api_key(KeyType::PostProcess, Some(provider_id)).unwrap_or_default()
}

/// Set a post-processing API key for a specific provider
pub fn set_post_process_api_key(provider_id: &str, key: &str) -> Result<()> {
    set_api_key(KeyType::PostProcess, Some(provider_id), key)
}

/// Get an AI Replace API key for a specific provider
pub fn get_ai_replace_api_key(provider_id: &str) -> String {
    #[cfg(not(target_os = "windows"))]
    if let Some(app) = APP_HANDLE.get() {
        return crate::settings::get_settings(app)
            .ai_replace_api_keys
            .get(provider_id)
            .cloned()
            .unwrap_or_default();
    }
    get_api_key(KeyType::AiReplace, Some(provider_id)).unwrap_or_default()
}

/// Set an AI Replace API key for a specific provider
pub fn set_ai_replace_api_key(provider_id: &str, key: &str) -> Result<()> {
    set_api_key(KeyType::AiReplace, Some(provider_id), key)
}

/// Get a Voice Command API key for a specific provider (returns Option for fallback logic)
pub fn get_voice_command_api_key(provider_id: &str) -> Option<String> {
    #[cfg(not(target_os = "windows"))]
    if let Some(app) = APP_HANDLE.get() {
        return crate::settings::get_settings(app)
            .voice_command_api_keys
            .get(provider_id)
            .cloned()
            .filter(|key| !key.is_empty());
    }
    get_api_key(KeyType::VoiceCommand, Some(provider_id))
        .ok()
        .filter(|k| !k.is_empty())
}

/// Set a Voice Command API key for a specific provider
pub fn set_voice_command_api_key(provider_id: &str, key: &str) -> Result<()> {
    set_api_key(KeyType::VoiceCommand, Some(provider_id), key)
}

/// Get Soniox STT API key
pub fn get_soniox_api_key() -> String {
    get_api_key(KeyType::SonioxStt, None).unwrap_or_default()
}

/// Set Soniox STT API key
pub fn set_soniox_api_key(key: &str) -> Result<()> {
    set_api_key(KeyType::SonioxStt, None, key)
}

/// Clear Soniox STT API key
pub fn clear_soniox_api_key() -> Result<()> {
    set_api_key(KeyType::SonioxStt, None, "")
}

/// Returns whether Soniox STT API key is present
pub fn has_soniox_api_key() -> bool {
    !get_soniox_api_key().trim().is_empty()
}

/// Get Deepgram STT API key
pub fn get_deepgram_api_key() -> String {
    get_api_key(KeyType::DeepgramStt, None).unwrap_or_default()
}

/// Set Deepgram STT API key
pub fn set_deepgram_api_key(key: &str) -> Result<()> {
    set_api_key(KeyType::DeepgramStt, None, key)
}

/// Clear Deepgram STT API key
pub fn clear_deepgram_api_key() -> Result<()> {
    set_api_key(KeyType::DeepgramStt, None, "")
}

/// Returns whether Deepgram STT API key is present
pub fn has_deepgram_api_key() -> bool {
    !get_deepgram_api_key().trim().is_empty()
}

/// Get a provider-specific Text-to-Speech API key.
pub fn get_tts_api_key(provider_id: &str) -> String {
    get_api_key(KeyType::Tts, Some(provider_id)).unwrap_or_default()
}

/// Store a provider-specific Text-to-Speech API key.
pub fn set_tts_api_key(provider_id: &str, key: &str) -> Result<()> {
    set_api_key(KeyType::Tts, Some(provider_id), key)
}

/// Clear a provider-specific Text-to-Speech API key.
pub fn clear_tts_api_key(provider_id: &str) -> Result<()> {
    set_api_key(KeyType::Tts, Some(provider_id), "")
}

/// Returns whether a provider-specific Text-to-Speech API key is present.
pub fn has_tts_api_key(provider_id: &str) -> bool {
    !get_tts_api_key(provider_id).trim().is_empty()
}

/// Get a provider-specific key used only by TTS LLM preprocessing.
pub fn get_tts_llm_api_key(provider_id: &str) -> String {
    get_api_key(KeyType::TtsLlm, Some(provider_id)).unwrap_or_default()
}

/// Store a provider-specific key used only by TTS LLM preprocessing.
pub fn set_tts_llm_api_key(provider_id: &str, key: &str) -> Result<()> {
    set_api_key(KeyType::TtsLlm, Some(provider_id), key)
}

/// Clear a provider-specific TTS LLM preprocessing key.
pub fn clear_tts_llm_api_key(provider_id: &str) -> Result<()> {
    set_api_key(KeyType::TtsLlm, Some(provider_id), "")
}

/// Returns whether a provider-specific TTS LLM preprocessing key is present.
pub fn has_tts_llm_api_key(provider_id: &str) -> bool {
    !get_tts_llm_api_key(provider_id).trim().is_empty()
}

// ============================================================================
// Migration from JSON settings to secure storage
// ============================================================================

/// Migrate API keys from JSON settings to Windows Credential Manager.
/// Returns true if any keys were migrated.
#[cfg(target_os = "windows")]
pub fn migrate_keys_from_settings(
    post_process_keys: &std::collections::HashMap<String, String>,
    ai_replace_keys: &std::collections::HashMap<String, String>,
    voice_command_keys: &std::collections::HashMap<String, String>,
) -> (bool, Vec<String>, Vec<String>, Vec<String>) {
    let mut migrated = false;
    let mut migrated_post_process = Vec::new();
    let mut migrated_ai_replace = Vec::new();
    let mut migrated_voice_command = Vec::new();

    // Migrate post-processing keys
    for (provider_id, key) in post_process_keys {
        if !key.trim().is_empty() {
            match set_post_process_api_key(provider_id, key) {
                Ok(()) => {
                    debug!(
                        "Migrated post-processing API key for provider: {}",
                        provider_id
                    );
                    migrated_post_process.push(provider_id.clone());
                    migrated = true;
                }
                Err(e) => {
                    warn!(
                        "Failed to migrate post-processing API key for {}: {}",
                        provider_id, e
                    );
                }
            }
        }
    }

    // Migrate AI Replace keys
    for (provider_id, key) in ai_replace_keys {
        if !key.trim().is_empty() {
            match set_ai_replace_api_key(provider_id, key) {
                Ok(()) => {
                    debug!("Migrated AI Replace API key for provider: {}", provider_id);
                    migrated_ai_replace.push(provider_id.clone());
                    migrated = true;
                }
                Err(e) => {
                    warn!(
                        "Failed to migrate AI Replace API key for {}: {}",
                        provider_id, e
                    );
                }
            }
        }
    }

    // Migrate Voice Command keys
    for (provider_id, key) in voice_command_keys {
        if !key.trim().is_empty() {
            match set_voice_command_api_key(provider_id, key) {
                Ok(()) => {
                    debug!(
                        "Migrated Voice Command API key for provider: {}",
                        provider_id
                    );
                    migrated_voice_command.push(provider_id.clone());
                    migrated = true;
                }
                Err(e) => {
                    warn!(
                        "Failed to migrate Voice Command API key for {}: {}",
                        provider_id, e
                    );
                }
            }
        }
    }

    (
        migrated,
        migrated_post_process,
        migrated_ai_replace,
        migrated_voice_command,
    )
}

#[cfg(not(target_os = "windows"))]
pub fn migrate_keys_from_settings(
    _post_process_keys: &std::collections::HashMap<String, String>,
    _ai_replace_keys: &std::collections::HashMap<String, String>,
    _voice_command_keys: &std::collections::HashMap<String, String>,
) -> (bool, Vec<String>, Vec<String>, Vec<String>) {
    // No migration on non-Windows platforms
    (false, Vec::new(), Vec::new(), Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_secret_family_has_a_stable_distinct_credential_namespace() {
        let cases = [
            (KeyType::PostProcess, "post_process_api_key"),
            (KeyType::AiReplace, "ai_replace_api_key"),
            (KeyType::VoiceCommand, "voice_command_api_key"),
            (KeyType::SonioxStt, "soniox_api_key"),
            (KeyType::DeepgramStt, "deepgram_api_key"),
            (KeyType::Tts, "tts_api_key"),
            (KeyType::TtsLlm, "tts_llm_api_key"),
        ];

        let mut prefixes = std::collections::HashSet::new();
        for (key_type, expected) in cases {
            assert_eq!(key_type.prefix(), expected);
            assert!(
                prefixes.insert(key_type.prefix()),
                "duplicate namespace {expected}"
            );
        }
    }

    #[test]
    fn provider_scoped_credentials_cannot_collide_with_global_credentials() {
        assert_eq!(KeyType::SonioxStt.credential_name(None), "soniox_api_key");
        assert_eq!(
            KeyType::Tts.credential_name(Some("openai")),
            "tts_api_key_openai"
        );
        assert_eq!(
            KeyType::TtsLlm.credential_name(Some("openai")),
            "tts_llm_api_key_openai"
        );
        assert_ne!(
            KeyType::Tts.credential_name(Some("openai")),
            KeyType::TtsLlm.credential_name(Some("openai"))
        );
    }
}
