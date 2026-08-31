export type SettingsSearchEntry = {
  id: string;
  section: string;
  anchor?: string;
  expandAnchor?: string;
  labelKey: string;
  fallbackLabel: string;
  groupLabelKey?: string;
  groupFallbackLabel?: string;
  unavailableReasonKey?: string;
  unavailableReasonFallback?: string;
  keywords: readonly string[];
};
