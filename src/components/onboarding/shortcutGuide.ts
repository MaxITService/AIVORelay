type ShortcutSettings = {
  push_to_talk: boolean;
  auto_shortcut_activation?: boolean;
  active_profile_id?: string;
  transcription_profiles?: { id: string; push_to_talk?: boolean; auto_shortcut_activation?: boolean }[];
};

export function getShortcutGuideMode(settings: ShortcutSettings | null): "hold" | "toggle" | "auto" | null {
  if (!settings) return null;
  const isDefault = !settings.active_profile_id || settings.active_profile_id === "default";
  const profile = isDefault ? undefined : settings.transcription_profiles?.find((p) => p.id === settings.active_profile_id);
  const auto = isDefault ? settings.auto_shortcut_activation : profile?.auto_shortcut_activation;
  const hold = profile?.push_to_talk ?? settings.push_to_talk;
  return auto ? "auto" : hold ? "hold" : "toggle";
}
