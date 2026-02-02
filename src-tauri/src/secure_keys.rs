//! Secure API key storage using Windows Credential Manager.
//!
//! On Windows, API keys are stored in the OS credential vault for security.
//! On other platforms, this module provides stub implementations that return errors,
//! as secure storage is Windows-only in this fork.

use anyhow::{anyhow, Result};
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
}

impl KeyType {
    fn prefix(&self) -> &'static str {
        match self {
            KeyType::PostProcess => "post_process_api_key",
            KeyType::AiReplace => "ai_replace_api_key",
            KeyType::VoiceCommand => "voice_command_api_key",
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
// Non-Windows stubs
// ============================================================================

#[cfg(not(target_os = "windows"))]
pub fn set_api_key(_key_type: KeyType, _provider_id: Option<&str>, _key: &str) -> Result<()> {
    Err(anyhow!("Secure key storage is only available on Windows"))
}

#[cfg(not(target_os = "windows"))]
pub fn get_api_key(_key_type: KeyType, _provider_id: Option<&str>) -> Result<String> {
    Err(anyhow!("Secure key storage is only available on Windows"))
}

// ============================================================================
// Convenience functions for specific key types
// ============================================================================

/// Get a post-processing API key for a specific provider
pub fn get_post_process_api_key(provider_id: &str) -> String {
    get_api_key(KeyType::PostProcess, Some(provider_id)).unwrap_or_default()
}

/// Set a post-processing API key for a specific provider
pub fn set_post_process_api_key(provider_id: &str, key: &str) -> Result<()> {
    set_api_key(KeyType::PostProcess, Some(provider_id), key)
}

/// Get an AI Replace API key for a specific provider
pub fn get_ai_replace_api_key(provider_id: &str) -> String {
    get_api_key(KeyType::AiReplace, Some(provider_id)).unwrap_or_default()
}

/// Set an AI Replace API key for a specific provider
pub fn set_ai_replace_api_key(provider_id: &str, key: &str) -> Result<()> {
    set_api_key(KeyType::AiReplace, Some(provider_id), key)
}

/// Get a Voice Command API key for a specific provider (returns Option for fallback logic)
pub fn get_voice_command_api_key(provider_id: &str) -> Option<String> {
    get_api_key(KeyType::VoiceCommand, Some(provider_id))
        .ok()
        .filter(|k| !k.is_empty())
}

/// Set a Voice Command API key for a specific provider
pub fn set_voice_command_api_key(provider_id: &str, key: &str) -> Result<()> {
    set_api_key(KeyType::VoiceCommand, Some(provider_id), key)
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
) -> (bool, Vec<String>, Vec<String>) {
    let mut migrated = false;
    let mut migrated_post_process = Vec::new();
    let mut migrated_ai_replace = Vec::new();

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

    (migrated, migrated_post_process, migrated_ai_replace)
}

#[cfg(not(target_os = "windows"))]
pub fn migrate_keys_from_settings(
    _post_process_keys: &std::collections::HashMap<String, String>,
    _ai_replace_keys: &std::collections::HashMap<String, String>,
) -> (bool, Vec<String>, Vec<String>) {
    // No migration on non-Windows platforms
    (false, Vec::new(), Vec::new())
}
