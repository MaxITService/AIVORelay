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
    binding_ids: Vec<String>,
    dynamic_prefixes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HotkeyGuideSection {
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
    let mut guide_bindings: Vec<ShortcutBinding> = settings.bindings.values().cloned().collect();
    if settings.text_replacement_decapitalize_after_edit_key_enabled {
        guide_bindings.push(ShortcutBinding {
            id: "text_replacement_decapitalize_after_edit_key".to_string(),
            name: "Decapitalize monitored key".to_string(),
            description: "Primary passive edit key used by Decapitalize After Manual Edit"
                .to_string(),
            default_binding: "backspace".to_string(),
            current_binding: settings
                .text_replacement_decapitalize_after_edit_key
                .clone(),
        });

        if settings.text_replacement_decapitalize_after_edit_secondary_key_enabled {
            guide_bindings.push(ShortcutBinding {
                id: "text_replacement_decapitalize_after_edit_secondary_key".to_string(),
                name: "Decapitalize secondary monitored key".to_string(),
                description:
                    "Secondary passive edit key used by Decapitalize After Manual Edit"
                        .to_string(),
                default_binding: "delete".to_string(),
                current_binding: settings
                    .text_replacement_decapitalize_after_edit_secondary_key
                    .clone(),
            });
        }
    }

    categories
        .into_iter()
        .filter_map(|category| {
            let binding_ids: HashSet<&str> =
                category.binding_ids.iter().map(|id| id.as_str()).collect();
            let bindings: Vec<_> = guide_bindings
                .iter()
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
                Some(HotkeyGuideSection { bindings })
            }
        })
        .collect()
}

fn is_binding_enabled_for_guide(
    settings: &AppSettings,
    feature_gates: &HashMap<String, String>,
    binding_id: &str,
) -> bool {
    if let Some(preset_id) =
        binding_id.strip_prefix(crate::settings::SEND_SELECTED_TEXT_BINDING_PREFIX)
    {
        return settings
            .send_selected_text
            .presets
            .iter()
            .any(|preset| preset.id == preset_id && preset.enabled);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{get_default_settings, SendSelectedTextPreset};

    fn binding(id: &str, current_binding: &str) -> ShortcutBinding {
        ShortcutBinding {
            id: id.to_string(),
            name: id.to_string(),
            description: String::new(),
            default_binding: String::new(),
            current_binding: current_binding.to_string(),
        }
    }

    #[test]
    fn bundled_manifest_has_unique_membership_and_only_supported_feature_gates() {
        let manifest: HotkeyGuideManifest =
            serde_json::from_str(HOTKEY_GUIDE_MANIFEST_JSON).unwrap();
        let supported_gates = HashSet::from([
            "send_to_extension_enabled",
            "send_to_extension_with_selection_enabled",
            "send_screenshot_to_extension_enabled",
            "voice_command_enabled",
        ]);
        assert!(manifest
            .feature_gates
            .values()
            .all(|gate| supported_gates.contains(gate.as_str())));

        let mut static_ids = HashSet::new();
        let mut dynamic_prefixes = HashSet::new();
        for category in manifest.categories {
            for id in category.binding_ids {
                assert!(static_ids.insert(id.clone()), "duplicate binding id {id}");
            }
            for prefix in category.dynamic_prefixes {
                assert!(
                    dynamic_prefixes.insert(prefix.clone()),
                    "duplicate dynamic prefix {prefix}"
                );
            }
        }
    }

    #[test]
    fn category_matching_limits_transcribe_prefix_to_real_profiles() {
        let no_static_ids = HashSet::new();
        let profile_ids = HashSet::from(["transcribe_profile-1".to_string()]);
        let prefixes = vec!["transcribe_".to_string()];

        assert!(is_binding_in_category(
            &binding("transcribe_profile-1", "ctrl+1"),
            &no_static_ids,
            &prefixes,
            &profile_ids,
        ));
        assert!(!is_binding_in_category(
            &binding("transcribe_deleted-profile", "ctrl+2"),
            &no_static_ids,
            &prefixes,
            &profile_ids,
        ));
        assert!(!is_binding_in_category(
            &binding("transcribe_default", "ctrl+3"),
            &no_static_ids,
            &prefixes,
            &profile_ids,
        ));
    }

    #[test]
    fn feature_gates_fail_closed_and_follow_the_corresponding_setting() {
        let mut settings = get_default_settings();
        let gates = HashMap::from([
            (
                "voice_command".to_string(),
                "voice_command_enabled".to_string(),
            ),
            ("future_binding".to_string(), "unknown_setting".to_string()),
        ]);

        settings.voice_command_enabled = false;
        assert!(!is_binding_enabled_for_guide(
            &settings,
            &gates,
            "voice_command"
        ));
        settings.voice_command_enabled = true;
        assert!(is_binding_enabled_for_guide(
            &settings,
            &gates,
            "voice_command"
        ));
        assert!(!is_binding_enabled_for_guide(
            &settings,
            &gates,
            "future_binding"
        ));
        assert!(is_binding_enabled_for_guide(
            &settings,
            &gates,
            "ungated_binding"
        ));
    }

    #[test]
    fn send_selected_text_hotkeys_require_an_existing_enabled_preset() {
        let mut settings = get_default_settings();
        settings.send_selected_text.presets = vec![SendSelectedTextPreset {
            id: "notes".to_string(),
            enabled: true,
            ..SendSelectedTextPreset::default()
        }];
        let gates = HashMap::new();

        assert!(is_binding_enabled_for_guide(
            &settings,
            &gates,
            "send_selected_text_notes"
        ));
        assert!(!is_binding_enabled_for_guide(
            &settings,
            &gates,
            "send_selected_text_deleted"
        ));

        settings.send_selected_text.presets[0].enabled = false;
        assert!(!is_binding_enabled_for_guide(
            &settings,
            &gates,
            "send_selected_text_notes"
        ));
    }

    #[test]
    fn guide_omits_blank_and_disabled_bindings_without_leaving_empty_sections() {
        let mut settings = get_default_settings();
        settings.bindings.clear();
        settings.transcription_profiles.clear();
        settings.bindings.insert(
            "voice_command".to_string(),
            binding("voice_command", "ctrl+shift+v"),
        );
        settings
            .bindings
            .insert("cancel".to_string(), binding("cancel", "   "));

        settings.voice_command_enabled = false;
        assert!(build_hotkey_guide_sections(&settings).is_empty());

        settings.voice_command_enabled = true;
        let sections = build_hotkey_guide_sections(&settings);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].bindings.len(), 1);
        assert_eq!(sections[0].bindings[0].id, "voice_command");
    }

    #[test]
    fn decapitalize_monitor_keys_are_injected_only_when_individually_enabled() {
        let mut settings = get_default_settings();
        settings.bindings.clear();
        settings.transcription_profiles.clear();
        settings.text_replacement_decapitalize_after_edit_key_enabled = true;
        settings.text_replacement_decapitalize_after_edit_secondary_key_enabled = false;
        settings.text_replacement_decapitalize_after_edit_key = "backspace".to_string();
        settings.text_replacement_decapitalize_after_edit_secondary_key = "delete".to_string();

        let primary_ids: Vec<_> = build_hotkey_guide_sections(&settings)
            .into_iter()
            .flat_map(|section| section.bindings)
            .map(|binding| binding.id)
            .collect();
        assert_eq!(
            primary_ids,
            vec!["text_replacement_decapitalize_after_edit_key"]
        );

        settings.text_replacement_decapitalize_after_edit_secondary_key_enabled = true;
        let both_ids: HashSet<_> = build_hotkey_guide_sections(&settings)
            .into_iter()
            .flat_map(|section| section.bindings)
            .map(|binding| binding.id)
            .collect();
        assert_eq!(
            both_ids,
            HashSet::from([
                "text_replacement_decapitalize_after_edit_key".to_string(),
                "text_replacement_decapitalize_after_edit_secondary_key".to_string(),
            ])
        );
    }
}
