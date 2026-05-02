use crate::settings::{AppSettings, ShortcutBinding};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

// Shared contract with src/lib/hotkeyGuide.ts. The JSON manifest owns category
// membership and feature gates; this Rust adapter only maps it onto AppSettings
// for native tray rendering.
const HOTKEY_GUIDE_MANIFEST_JSON: &str = include_str!("../../src/lib/hotkeyGuideManifest.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HotkeyGuideManifest {
    feature_gates: HashMap<String, String>,
    categories: Vec<HotkeyGuideCategory>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HotkeyGuideCategory {
    title: String,
    binding_ids: Vec<String>,
    dynamic_prefixes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HotkeyGuideSection {
    pub title: String,
    pub bindings: Vec<ShortcutBinding>,
}

pub fn build_hotkey_guide_sections(settings: &AppSettings) -> Vec<HotkeyGuideSection> {
    let Ok(manifest) = serde_json::from_str::<HotkeyGuideManifest>(HOTKEY_GUIDE_MANIFEST_JSON)
    else {
        log::warn!("Failed to parse bundled hotkey guide manifest.");
        return Vec::new();
    };

    let profile_binding_ids: HashSet<String> = settings
        .transcription_profiles
        .iter()
        .map(|profile| format!("transcribe_{}", profile.id))
        .collect();
    let HotkeyGuideManifest {
        feature_gates,
        categories,
    } = manifest;

    categories
        .into_iter()
        .filter_map(|category| {
            let binding_ids: HashSet<&str> =
                category.binding_ids.iter().map(|id| id.as_str()).collect();
            let bindings: Vec<_> = settings
                .bindings
                .values()
                .filter(|binding| {
                    !binding.current_binding.trim().is_empty()
                        && is_binding_enabled_for_guide(settings, &feature_gates, &binding.id)
                        && is_binding_in_category(
                            binding,
                            &binding_ids,
                            &category.dynamic_prefixes,
                            &profile_binding_ids,
                        )
                })
                .cloned()
                .collect();

            if bindings.is_empty() {
                None
            } else {
                Some(HotkeyGuideSection {
                    title: category.title,
                    bindings,
                })
            }
        })
        .collect()
}

fn is_binding_enabled_for_guide(
    settings: &AppSettings,
    feature_gates: &HashMap<String, String>,
    binding_id: &str,
) -> bool {
    let Some(setting_key) = feature_gates.get(binding_id) else {
        return true;
    };

    match setting_key.as_str() {
        "send_to_extension_enabled" => settings.send_to_extension_enabled,
        "send_to_extension_with_selection_enabled" => {
            settings.send_to_extension_with_selection_enabled
        }
        "send_screenshot_to_extension_enabled" => settings.send_screenshot_to_extension_enabled,
        "voice_command_enabled" => settings.voice_command_enabled,
        unknown => {
            log::warn!(
                "Unknown hotkey guide feature gate setting '{}' for binding '{}'.",
                unknown,
                binding_id
            );
            false
        }
    }
}

fn is_binding_in_category(
    binding: &ShortcutBinding,
    binding_ids: &HashSet<&str>,
    dynamic_prefixes: &[String],
    profile_binding_ids: &HashSet<String>,
) -> bool {
    if binding_ids.contains(binding.id.as_str()) {
        return true;
    }

    dynamic_prefixes.iter().any(|prefix| {
        if !binding.id.starts_with(prefix) {
            return false;
        }
        if prefix == "transcribe_" {
            return profile_binding_ids.contains(&binding.id);
        }
        true
    })
}
