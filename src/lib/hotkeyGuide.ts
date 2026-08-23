import type {
  AppSettings,
  ShortcutBinding,
  TranscriptionProfile,
} from "@/bindings";
import manifest from "./hotkeyGuideManifest.json";

// Shared contract for every UI surface that displays the app's assigned buttons.
// Keep category membership and feature gates in hotkeyGuideManifest.json so the
// right sidebar and native tray guide cannot drift independently.
export interface HotkeyGuideCategory {
  id: string;
  title: string;
  titleKey: string;
  bindingIds: string[];
  dynamicPrefixes: string[];
}

export interface HotkeyGuideManifest {
  version: number;
  featureGates: Record<string, keyof AppSettings>;
  categories: HotkeyGuideCategory[];
}

export interface HotkeyGuideCategoryItems {
  id: string;
  titleKey: string;
  hotkeys: ShortcutBinding[];
}

export const hotkeyGuideManifest = manifest as HotkeyGuideManifest;

const isFeatureEnabled = (
  hotkeyId: string,
  settings: AppSettings | null,
): boolean => {
  if (hotkeyId.startsWith("send_selected_text_") && settings) {
    const presetId = hotkeyId.slice("send_selected_text_".length);
    const feature = (
      settings as AppSettings & {
        send_selected_text?: {
          presets?: Array<{ id: string; enabled: boolean }>;
        };
      }
    ).send_selected_text;
    return Boolean(
      feature?.presets?.some(
        (preset) => preset.id === presetId && preset.enabled,
      ),
    );
  }
  const settingKey = hotkeyGuideManifest.featureGates[hotkeyId];
  if (!settingKey || !settings) return true;
  return Boolean(settings[settingKey]);
};

const profileBindingIds = (profiles: TranscriptionProfile[]): Set<string> =>
  new Set(profiles.map((profile) => `transcribe_${profile.id}`));

const decapitalizeMonitorBindings = (
  settings: AppSettings | null,
): ShortcutBinding[] => {
  if (!settings?.text_replacement_decapitalize_after_edit_key_enabled) {
    return [];
  }

  const bindings: ShortcutBinding[] = [
    {
      id: "text_replacement_decapitalize_after_edit_key",
      name: "Decapitalize monitored key",
      description:
        "Primary passive edit key used by Decapitalize After Manual Edit",
      default_binding: "backspace",
      current_binding:
        settings.text_replacement_decapitalize_after_edit_key ?? "backspace",
    },
  ];

  if (settings.text_replacement_decapitalize_after_edit_secondary_key_enabled) {
    bindings.push({
      id: "text_replacement_decapitalize_after_edit_secondary_key",
      name: "Decapitalize secondary monitored key",
      description:
        "Secondary passive edit key used by Decapitalize After Manual Edit",
      default_binding: "delete",
      current_binding:
        settings.text_replacement_decapitalize_after_edit_secondary_key ??
        "delete",
    });
  }

  return bindings;
};

export const buildHotkeyGuideCategories = (
  bindings: Record<string, ShortcutBinding>,
  profiles: TranscriptionProfile[],
  settings: AppSettings | null,
): HotkeyGuideCategoryItems[] => {
  const assigned = [
    ...Object.values(bindings),
    ...decapitalizeMonitorBindings(settings),
  ].filter(
    (binding) =>
      Boolean(binding.current_binding?.trim()) &&
      isFeatureEnabled(binding.id, settings),
  );
  const profileIds = profileBindingIds(profiles);

  return hotkeyGuideManifest.categories
    .map((category) => {
      const categoryBindingIds = new Set(category.bindingIds);
      const hotkeys = assigned.filter((binding) => {
        if (categoryBindingIds.has(binding.id)) {
          return true;
        }
        return category.dynamicPrefixes.some((prefix) => {
          if (!binding.id.startsWith(prefix)) {
            return false;
          }
          return prefix === "transcribe_" ? profileIds.has(binding.id) : true;
        });
      });

      return {
        id: category.id,
        titleKey: category.titleKey,
        hotkeys,
      };
    })
    .filter((category) => category.hotkeys.length > 0);
};
