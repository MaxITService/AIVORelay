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
  const settingKey = hotkeyGuideManifest.featureGates[hotkeyId];
  if (!settingKey || !settings) return true;
  return Boolean(settings[settingKey]);
};

const profileBindingIds = (profiles: TranscriptionProfile[]): Set<string> =>
  new Set(profiles.map((profile) => `transcribe_${profile.id}`));

export const buildHotkeyGuideCategories = (
  bindings: Record<string, ShortcutBinding>,
  profiles: TranscriptionProfile[],
  settings: AppSettings | null,
): HotkeyGuideCategoryItems[] => {
  const assigned = Object.values(bindings).filter(
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
