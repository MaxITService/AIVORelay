import { useCallback, useEffect, useState } from "react";
import type { AppSettings } from "@/bindings";
import { useSettingsStore } from "../stores/settingsStore";

export type StringSettingKey = {
  [K in keyof AppSettings]: NonNullable<AppSettings[K]> extends string ? K : never;
}[keyof AppSettings];

export const usePersistedSettingText = <K extends StringSettingKey>(settingKey: K) => {
  const persistedSetting = useSettingsStore((state) => state.settings?.[settingKey]);
  const updateSetting = useSettingsStore((state) => state.updateSetting);
  const persistedValue = (persistedSetting as string | undefined) ?? "";
  const [draft, setDraft] = useState(persistedValue);

  useEffect(() => {
    setDraft(persistedValue);
  }, [persistedValue]);

  const persistDraft = useCallback(() => {
    if (draft !== persistedValue) {
      void updateSetting(settingKey, draft as AppSettings[K]);
    }
  }, [draft, persistedValue, settingKey, updateSetting]);

  return { draft, setDraft, persistDraft };
};
