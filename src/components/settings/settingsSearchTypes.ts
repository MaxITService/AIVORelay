import type { AppSettings } from "@/bindings";

export type SettingsSearchContext = {
  isPortable: boolean | null;
};

export type SettingsSearchEntry = {
  id: string;
  section: string;
  anchor?: string;
  fallbackAnchor?: string;
  expandAnchor?: string;
  labelKey: string;
  fallbackLabel: string;
  groupLabelKey?: string;
  groupFallbackLabel?: string;
  unavailableReasonKey?: string;
  unavailableReasonFallback?: string;
  isAvailable?: (
    settings: AppSettings | null,
    context?: SettingsSearchContext,
  ) => boolean;
  keywords: readonly string[];
};
