import React, { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  AlertTriangle,
  ArrowRight,
  ArrowUpDown,
  CaseSensitive,
  Check,
  ChevronDown,
  ChevronUp,
  Download,
  HelpCircle,
  Loader2,
  Plus,
  Regex,
  Search,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { type as getOsType } from "@tauri-apps/plugin-os";
import { useSettings } from "@/hooks/useSettings";
import { useNavigationStore } from "@/stores/navigationStore";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { ToggleSwitch } from "@/components/ui/ToggleSwitch";
import { Tooltip } from "@/components/ui/Tooltip";
import { CustomWords } from "@/components/settings/CustomWords";
import { Slider } from "@/components/ui/Slider";
import { TellMeMore } from "@/components/ui/TellMeMore";
import { HotkeyCapture } from "@/components/ui/HotkeyCapture";
import { formatKeyCombination, type OSType } from "@/lib/utils/keyboard";
import { getShortcutAnchorId } from "@/lib/shortcutAnchors";
import { getActiveProfilePostProcessingEnabled } from "@/lib/postProcessingAvailability";
import { sessionToast as toast } from "@/lib/sessionToast";
import { TextReplacementImportDialog } from "./TextReplacementImportDialog";
import {
  getVisibleTextReplacementRules,
  type TextReplacementColumnSortDirection,
  type TextReplacementRule,
  type TextReplacementSearchScope,
  type TextReplacementSortOrder,
} from "./textReplacementRuleView";
import {
  applyTextReplacementImport,
  parseTextReplacementRulesJson,
  serializeTextReplacementRules,
  TextReplacementTransferError,
  type TextReplacementImportMode,
  type TextReplacementImportResult,
} from "./textReplacementRuleTransfer";

type OutputWhitespaceMode = "preserve" | "remove_if_present" | "add_if_missing";

const MODIFIER_SHORTCUT_TOKENS = new Set([
  "ctrl",
  "control",
  "shift",
  "alt",
  "option",
  "win",
  "windows",
  "meta",
  "cmd",
  "command",
  "super",
]);

const splitShortcutTokens = (binding: string): string[] =>
  binding
    .split("+")
    .map((part) => part.trim().toLowerCase())
    .filter((part) => part.length > 0);

interface ColumnSortControlProps {
  column: "find" | "replacement";
  direction: TextReplacementColumnSortDirection;
  onChange: (direction: TextReplacementColumnSortDirection) => void;
  onTurnOff: () => void;
}

function ColumnSortControl({
  column,
  direction,
  onChange,
  onTurnOff,
}: ColumnSortControlProps) {
  const { t } = useTranslation();
  const isFindColumn = column === "find";
  const label = isFindColumn
    ? t("textReplacement.findTextColumn", "Find Text")
    : t("textReplacement.replaceWithColumn", "Replace with");
  const ariaLabel = isFindColumn
    ? t("textReplacement.sortFindTextLabel", "Sort Find Text")
    : t("textReplacement.sortReplaceWithLabel", "Sort Replace with");
  const offTooltip = t(
    "textReplacement.sortOffTooltip",
    "Off restores the order in which rules were added, oldest first. Shift-click either sorting control to turn sorting off.",
  );
  const activeTooltip = t(
    "textReplacement.sortShiftClickTooltip",
    "Shift-click to turn sorting off.",
  );
  const handleMouseDown = (event: React.MouseEvent<HTMLSelectElement>) => {
    if (!event.shiftKey) return;
    event.preventDefault();
    onTurnOff();
    event.currentTarget.blur();
  };

  return (
    <div className="flex min-w-0 flex-1 flex-col items-start gap-1 sm:flex-row sm:items-center sm:justify-between sm:gap-2">
      <span className="truncate text-xs font-medium text-[#a8a8a8]">
        {label}
      </span>
      <Tooltip
        content={direction === "off" ? offTooltip : activeTooltip}
        position="top"
      >
        <span className="relative inline-block w-[5.75rem] shrink-0">
          <ArrowUpDown
            className={`pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 ${
              direction === "off" ? "text-[#606060]" : "text-[#c69cff]"
            }`}
            aria-hidden="true"
          />
          <select
            value={direction}
            onMouseDown={handleMouseDown}
            onChange={(event) =>
              onChange(
                event.target.value as TextReplacementColumnSortDirection,
              )
            }
            aria-label={ariaLabel}
            className={`min-h-7 w-full appearance-none rounded-md border bg-[#181818] py-1 pl-7 pr-7 text-xs outline-none transition-colors focus:border-[#9b5de5] [color-scheme:dark] ${
              direction === "off"
                ? "border-[#3c3c3c] text-[#8a8a8a] hover:border-[#505050]"
                : "border-[#9b5de5] text-[#c69cff]"
            }`}
          >
            <option value="off" title={offTooltip}>
              {t("textReplacement.sortOff", "Off")}
            </option>
            <option value="asc">
              {t("textReplacement.sortAscending", "A → Z")}
            </option>
            <option value="desc">
              {t("textReplacement.sortDescending", "Z → A")}
            </option>
          </select>
          <ChevronDown
            className="pointer-events-none absolute right-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[#707070]"
            aria-hidden="true"
          />
        </span>
      </Tooltip>
    </div>
  );
}

export const TextReplacementSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating } = useSettings();
  const { setSection } = useNavigationStore();
  type DecapCaptureTarget = "primary" | "secondary";

  const [newFrom, setNewFrom] = useState("");
  const [newTo, setNewTo] = useState("");
  const [newCaseSensitive, setNewCaseSensitive] = useState(true);
  const [newIsRegex, setNewIsRegex] = useState(false);
  const [showHelp, setShowHelp] = useState(false);
  const [ruleSearchQuery, setRuleSearchQuery] = useState("");
  const [ruleSearchScope, setRuleSearchScope] =
    useState<TextReplacementSearchScope>("all");
  const [ruleSortOrder, setRuleSortOrder] =
    useState<TextReplacementSortOrder>("added");
  const [ruleTransferBusy, setRuleTransferBusy] = useState<
    "import" | "export" | null
  >(null);
  const ruleTransferBusyRef = useRef<"import" | "export" | null>(null);
  const [pendingImportRules, setPendingImportRules] = useState<
    TextReplacementRule[] | null
  >(null);
  const [importMode, setImportMode] =
    useState<TextReplacementImportMode>("merge");
  const [overwriteImportConflicts, setOverwriteImportConflicts] =
    useState(false);

  // Editing state
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editFrom, setEditFrom] = useState("");
  const [editTo, setEditTo] = useState("");
  const [capturingDecapTarget, setCapturingDecapTarget] =
    useState<DecapCaptureTarget | null>(null);
  const [decapCaptureError, setDecapCaptureError] = useState<{
    target: DecapCaptureTarget;
    message: string;
  } | null>(null);

  const osKind = getOsType();
  const hotkeyOsType: OSType =
    osKind === "windows" || osKind === "macos" || osKind === "linux"
      ? osKind
      : "unknown";

  const replacements = useMemo<TextReplacementRule[]>(
    () =>
      (settings?.text_replacements ?? []).map((rule: any) => ({
        ...rule,
        enabled: rule.enabled ?? true,
        case_sensitive: rule.case_sensitive ?? true,
        is_regex: rule.is_regex ?? false,
      })),
    [settings?.text_replacements],
  );
  const latestReplacementsRef = useRef(replacements);
  useEffect(() => {
    latestReplacementsRef.current = replacements;
  }, [replacements]);
  const visibleReplacements = useMemo(
    () =>
      getVisibleTextReplacementRules(
        replacements,
        ruleSearchQuery,
        ruleSearchScope,
        ruleSortOrder,
      ),
    [replacements, ruleSearchQuery, ruleSearchScope, ruleSortOrder],
  );
  const filteredReplacementCount = Math.max(
    0,
    replacements.length - visibleReplacements.length,
  );
  const findSortDirection: TextReplacementColumnSortDirection =
    ruleSortOrder === "find-asc"
      ? "asc"
      : ruleSortOrder === "find-desc"
        ? "desc"
        : "off";
  const replacementSortDirection: TextReplacementColumnSortDirection =
    ruleSortOrder === "replacement-asc"
      ? "asc"
      : ruleSortOrder === "replacement-desc"
        ? "desc"
        : "off";
  const setColumnSort = (
    column: "find" | "replacement",
    direction: TextReplacementColumnSortDirection,
  ) => {
    setRuleSortOrder(
      direction === "off"
        ? "added"
        : (`${column}-${direction}` as TextReplacementSortOrder),
    );
  };
  const isEnabled = settings?.text_replacements_enabled ?? false;
  const decapitalizeAfterEditEnabled =
    settings?.text_replacement_decapitalize_after_edit_key_enabled ?? false;
  const decapitalizeAfterEditKey =
    settings?.text_replacement_decapitalize_after_edit_key ?? "backspace";
  const decapitalizeAfterEditSecondaryKeyEnabled =
    settings?.text_replacement_decapitalize_after_edit_secondary_key_enabled ?? false;
  const decapitalizeAfterEditSecondaryKey =
    settings?.text_replacement_decapitalize_after_edit_secondary_key ?? "delete";
  const decapitalizeAfterEditTimeoutMs =
    settings?.text_replacement_decapitalize_timeout_ms ?? 5000;
  const decapitalizeStandardPostRecordingMonitorMs =
    settings?.text_replacement_decapitalize_standard_post_recording_monitor_ms ?? 5000;
  const configuredShortcutEngine = (settings as any)?.shortcut_engine ?? "handy_keys";
  const leadingWhitespaceMode =
    (settings?.output_whitespace_leading_mode ?? "remove_if_present") as OutputWhitespaceMode;
  const trailingWhitespaceMode =
    (settings?.output_whitespace_trailing_mode ?? "remove_if_present") as OutputWhitespaceMode;

  const setLeadingWhitespaceMode = (mode: OutputWhitespaceMode) =>
    (updateSetting as any)("output_whitespace_leading_mode", mode);
  const setTrailingWhitespaceMode = (mode: OutputWhitespaceMode) =>
    (updateSetting as any)("output_whitespace_trailing_mode", mode);

  const saveDecapMonitoredKey = async (
    target: DecapCaptureTarget,
    settingKey:
      | "text_replacement_decapitalize_after_edit_key"
      | "text_replacement_decapitalize_after_edit_secondary_key",
    hotkey: string,
  ) => {
    setDecapCaptureError(null);
    try {
      await updateSetting(settingKey, hotkey, { throwOnError: true });
      setCapturingDecapTarget(null);
    } catch (error) {
      console.error("Failed to save monitored key:", error);
      const errorText = error instanceof Error ? error.message : String(error);
      const unsupportedKey = /unknown key|unsupported|not supported/i.test(
        errorText,
      );
      setDecapCaptureError({
        target,
        message: unsupportedKey
          ? t(
              "textReplacement.decapitalizeAfterEditUnsupportedKey",
              "This key is not supported. The previous monitored key was restored. Press another key or Escape to cancel.",
            )
          : t(
              "textReplacement.decapitalizeAfterEditKeySaveFailed",
              "This monitored key could not be saved. The previous key was restored. {{error}}",
              { error: errorText },
            ),
      });
    }
  };

  const monitoredDecapBindings = useMemo(() => {
    const bindings = [decapitalizeAfterEditKey];
    if (decapitalizeAfterEditSecondaryKeyEnabled) {
      bindings.push(decapitalizeAfterEditSecondaryKey);
    }

    return Array.from(
      new Set(bindings.map((binding) => binding.trim()).filter((binding) => binding.length > 0))
    );
  }, [
    decapitalizeAfterEditKey,
    decapitalizeAfterEditSecondaryKeyEnabled,
    decapitalizeAfterEditSecondaryKey,
  ]);

  const decapConflicts = useMemo(() => {
    const monitoredBindingsParsed = monitoredDecapBindings
      .map((binding) => {
        const monitoredTokens = splitShortcutTokens(binding);
        const monitoredMainKey =
          monitoredTokens.find((token) => !MODIFIER_SHORTCUT_TOKENS.has(token)) ?? null;
        const monitoredModifiers = monitoredTokens.filter((token) =>
          MODIFIER_SHORTCUT_TOKENS.has(token)
        );

        if (!monitoredMainKey && monitoredModifiers.length === 0) {
          return null;
        }

        return { mainKey: monitoredMainKey, modifiers: monitoredModifiers };
      })
      .filter((item): item is { mainKey: string | null; modifiers: string[] } => item !== null);

    if (monitoredBindingsParsed.length === 0) {
      return [];
    }

    const bindings = settings?.bindings ?? {};
    const conflicts: Array<{ id: string; name: string; binding: string }> = [];

    for (const [id, binding] of Object.entries(bindings)) {
      const currentBinding = binding?.current_binding?.trim() ?? "";
      if (!currentBinding) continue;

      const otherTokens = splitShortcutTokens(currentBinding);
      if (otherTokens.length === 0) continue;

      const overlaps = monitoredBindingsParsed.some((monitoredBinding) =>
        monitoredBinding.mainKey
          ? otherTokens.includes(monitoredBinding.mainKey)
          : monitoredBinding.modifiers.length > 0 &&
            monitoredBinding.modifiers.every((mod) => otherTokens.includes(mod))
      );

      if (!overlaps) continue;

      const displayName = t(
        `settings.general.shortcut.bindings.${id}.name`,
        binding?.name || id
      );

      conflicts.push({
        id,
        name: displayName,
        binding: currentBinding,
      });
    }

    conflicts.sort((a, b) => a.name.localeCompare(b.name));
    return conflicts;
  }, [monitoredDecapBindings, settings?.bindings, t]);

  useEffect(() => {
    if (!decapitalizeAfterEditEnabled) {
      setCapturingDecapTarget(null);
      setDecapCaptureError(null);
      return;
    }

    if (
      !decapitalizeAfterEditSecondaryKeyEnabled &&
      capturingDecapTarget === "secondary"
    ) {
      setCapturingDecapTarget(null);
    }
  }, [
    decapitalizeAfterEditEnabled,
    decapitalizeAfterEditSecondaryKeyEnabled,
    capturingDecapTarget,
  ]);

  const handleAddRule = () => {
    if (newFrom.length === 0) return;

    const newRule: TextReplacementRule = {
      id: `tr_${Date.now()}`,
      from: newFrom,
      to: newTo,
      enabled: true,
      case_sensitive: newCaseSensitive,
      is_regex: newIsRegex,
    };

    updateSetting("text_replacements", [...replacements, newRule]);
    setNewFrom("");
    setNewTo("");
  };

  const handleRemoveRule = (id: string) => {
    updateSetting(
      "text_replacements",
      replacements.filter((r) => r.id !== id)
    );
  };

  const handleToggleRule = (id: string) => {
    updateSetting(
      "text_replacements",
      replacements.map((r) =>
        r.id === id ? { ...r, enabled: !r.enabled } : r
      )
    );
  };

  const handleToggleCaseSensitive = (id: string) => {
    updateSetting(
      "text_replacements",
      replacements.map((r) =>
        r.id === id ? { ...r, case_sensitive: !r.case_sensitive } : r
      )
    );
  };

  const handleToggleIsRegex = (id: string) => {
    updateSetting(
      "text_replacements",
      replacements.map((r) =>
        r.id === id ? { ...r, is_regex: !r.is_regex } : r
      )
    );
  };

  const startEditing = (rule: TextReplacementRule) => {
    setEditingId(rule.id);
    setEditFrom(rule.from);
    setEditTo(rule.to);
  };

  const cancelEditing = () => {
    setEditingId(null);
    setEditFrom("");
    setEditTo("");
  };

  const saveEditing = () => {
    if (!editingId || editFrom.length === 0) return;
    
    updateSetting(
      "text_replacements",
      replacements.map((r) =>
        r.id === editingId ? { ...r, from: editFrom, to: editTo } : r
      )
    );
    setEditingId(null);
    setEditFrom("");
    setEditTo("");
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && newFrom.length > 0) {
      e.preventDefault();
      handleAddRule();
    }
  };

  const handleEditKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      saveEditing();
    } else if (e.key === "Escape") {
      cancelEditing();
    }
  };

  const getTransferErrorMessage = (
    operation: "import" | "export",
    error: unknown,
  ): string => {
    if (operation === "import" && error instanceof TextReplacementTransferError) {
      switch (error.code) {
        case "invalid-json":
          return t(
            "textReplacement.importInvalidJson",
            "The selected file is not valid JSON.",
          );
        case "unsupported-format":
          return t(
            "textReplacement.importUnsupportedFormat",
            "The selected file has an unsupported replacement-rules format.",
          );
        case "unsupported-version":
          return t(
            "textReplacement.importUnsupportedVersion",
            "The selected replacement-rules file uses an unsupported version.",
          );
        case "invalid-document":
        case "invalid-rules":
        case "invalid-rule":
          return t(
            "textReplacement.importInvalidDocument",
            "The selected file does not contain valid replacement rules.",
          );
        case "id-generation":
          return t(
            "textReplacement.importIdGenerationFailed",
            "Could not create unique IDs for the imported rules.",
          );
      }
    }

    return t(
      operation === "import"
        ? "textReplacement.importError"
        : "textReplacement.exportError",
      operation === "import"
        ? "Could not import replacement rules."
        : "Could not export replacement rules.",
    );
  };

  const beginRuleTransfer = (
    operation: "import" | "export",
    allowPendingImport = false,
  ): boolean => {
    if (
      ruleTransferBusyRef.current !== null ||
      isUpdating("text_replacements") ||
      (!allowPendingImport && pendingImportRules !== null)
    ) {
      return false;
    }

    ruleTransferBusyRef.current = operation;
    setRuleTransferBusy(operation);
    return true;
  };

  const endRuleTransfer = () => {
    ruleTransferBusyRef.current = null;
    setRuleTransferBusy(null);
  };

  const createImportIdFactory = () => {
    const timestamp = Date.now();
    let counter = 0;

    return (_originalId: string, attempt: number) => {
      const suffix = counter++;
      return `tr_import_${timestamp}_${suffix}_${attempt}`;
    };
  };

  const showImportResult = (
    mode: TextReplacementImportMode,
    result: TextReplacementImportResult,
  ) => {
    if (mode === "replace") {
      toast.success(
        t("textReplacement.importReplaceSuccess", {
          imported: result.importedCount,
          total: result.rules.length,
          skipped: result.skippedDuplicateCount,
          remapped: result.remappedIdCount,
        }),
      );
      return;
    }

    toast.success(
      t("textReplacement.importMergeSuccess", {
        added: result.addedCount,
        overwritten: result.overwrittenConflictCount,
        duplicates: result.skippedDuplicateCount,
        conflicts: result.skippedConflictCount,
        remapped: result.remappedIdCount,
      }),
    );
  };

  const showImportNoChanges = (result: TextReplacementImportResult) => {
    if (result.skippedConflictCount === 0) {
      toast.info(
        t(
          "textReplacement.importNoChanges",
          "All imported rules were already present. No changes were made.",
        ),
      );
      return;
    }

    toast.info(
      t("textReplacement.importMergeNoChanges", {
        duplicates: result.skippedDuplicateCount,
        conflicts: result.skippedConflictCount,
      }),
    );
  };

  const resetPendingImport = () => {
    setPendingImportRules(null);
    setImportMode("merge");
    setOverwriteImportConflicts(false);
  };

  const handleExportRules = async () => {
    if (
      !settings ||
      replacements.length === 0 ||
      !beginRuleTransfer("export")
    ) {
      return;
    }

    try {
      const destination = await save({
        defaultPath: "aivorelay-text-replacements.json",
        filters: [
          {
            name: t("textReplacement.jsonFiles", "JSON files"),
            extensions: ["json"],
          },
        ],
      });
      if (!destination) return;

      await writeTextFile(destination, serializeTextReplacementRules(replacements));
      toast.success(
        t(
          "textReplacement.exportSuccess",
          "Replacement rules exported successfully.",
        ),
      );
    } catch (error) {
      console.error("Failed to export replacement rules:", error);
      toast.error(getTransferErrorMessage("export", error));
    } finally {
      endRuleTransfer();
    }
  };

  const handleImportRules = async () => {
    if (!settings || !beginRuleTransfer("import")) return;

    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [
          {
            name: t("textReplacement.jsonFiles", "JSON files"),
            extensions: ["json"],
          },
        ],
      });
      if (typeof selected !== "string") return;

      const importedRules = parseTextReplacementRulesJson(
        await readTextFile(selected),
      );
      if (importedRules.length === 0) {
        toast.info(
          t(
            "textReplacement.importEmpty",
            "The selected file contains no replacement rules. No changes were made.",
          ),
        );
        return;
      }

      const currentRules = latestReplacementsRef.current;
      if (currentRules.length === 0) {
        const replaceResult = applyTextReplacementImport([], importedRules, {
          mode: "replace",
          overwriteConflicts: false,
          idFactory: createImportIdFactory(),
        });
        await updateSetting("text_replacements", replaceResult.rules, {
          throwOnError: true,
        });
        showImportResult("replace", replaceResult);
        return;
      }

      setPendingImportRules(importedRules);
      setImportMode("merge");
      setOverwriteImportConflicts(false);
    } catch (error) {
      console.error("Failed to import replacement rules:", error);
      toast.error(getTransferErrorMessage("import", error));
    } finally {
      endRuleTransfer();
    }
  };

  const handleCancelImport = () => {
    if (ruleTransferBusyRef.current !== null) return;
    resetPendingImport();
  };

  const handleConfirmImport = async () => {
    const stagedRules = pendingImportRules;
    if (
      !settings ||
      stagedRules === null ||
      !beginRuleTransfer("import", true)
    ) {
      return;
    }

    const selectedMode = importMode;
    try {
      const result = applyTextReplacementImport(replacements, stagedRules, {
        mode: selectedMode,
        overwriteConflicts:
          selectedMode === "merge" && overwriteImportConflicts,
        idFactory: createImportIdFactory(),
      });

      if (
        selectedMode === "merge" &&
        result.importedCount === 0 &&
        result.remappedIdCount === 0
      ) {
        resetPendingImport();
        showImportNoChanges(result);
        return;
      }

      await updateSetting("text_replacements", result.rules, {
        throwOnError: true,
      });
      showImportResult(selectedMode, result);
      resetPendingImport();
    } catch (error) {
      console.error("Failed to import replacement rules:", error);
      toast.error(getTransferErrorMessage("import", error));
    } finally {
      endRuleTransfer();
    }
  };

  // Format display text to show escape sequences visually
  const formatDisplayText = (text: string): string => {
    if (!text) return t("textReplacement.emptyValue", "(empty)");
    return text
      .replace(/\n/g, "⏎")
      .replace(/\r/g, "↵")
      .replace(/\t/g, "⇥");
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6 pb-12">
      <SettingsGroup
        title={t(
          "textReplacement.decapitalizeAfterEditTitle",
          "Decapitalize After Manual Edit"
        )}
        description={t(
          "textReplacement.decapitalizeAfterEditGroupDescription",
          "Independent feature: monitors your edit key and can lowercase the next suitable transcript chunk even when Text Replacement rules are disabled."
        )}
      >
        <div className="px-4 py-3">
          <ToggleSwitch
            checked={decapitalizeAfterEditEnabled}
            onChange={(enabled) =>
              updateSetting(
                "text_replacement_decapitalize_after_edit_key_enabled",
                enabled
              )
            }
            isUpdating={isUpdating("text_replacement_decapitalize_after_edit_key_enabled")}
            label={t(
              "textReplacement.decapitalizeAfterEditEnableLabel",
              "Enable Decapitalize After Edit Key"
            )}
            description={t(
              "textReplacement.decapitalizeAfterEditEnableDescription",
              "Use a passive keyboard hook (non-blocking) to detect manual edits and lowercase the next matching chunk."
            )}
            descriptionMode="inline"
          />
        </div>

        {decapitalizeAfterEditEnabled && (
          <>
            {hotkeyOsType === "windows" && (
              <div className="px-4 py-3 border-t border-white/[0.05]">
                <div
                  role="alert"
                  className="rounded-md border border-red-500/40 bg-red-500/10 px-3 py-3"
                >
                  <div className="flex items-start gap-2">
                    <AlertTriangle className="mt-0.5 h-4 w-4 flex-shrink-0 text-red-300" />
                    <div className="space-y-2 text-xs">
                      <div className="font-semibold text-red-200">
                        {t(
                          "textReplacement.decapitalizeAfterEditRdevWarningTitle",
                          "Warning: this feature uses passive rdev monitoring"
                        )}
                      </div>
                      <p className="text-red-100/90">
                        {t(
                          "textReplacement.decapitalizeAfterEditRdevWarningBody",
                          "Only rdev can passively monitor keys like Backspace or Delete while the active application still receives those keys normally."
                        )}
                      </p>
                      <p className="text-red-100/80">
                        {configuredShortcutEngine === "rdev"
                          ? t(
                              "textReplacement.decapitalizeAfterEditRdevWarningRdevBody",
                              "Your main Shortcut Engine is already rdev, so turning this feature off disables only this decapitalize monitor."
                            )
                          : t(
                              "textReplacement.decapitalizeAfterEditRdevWarningNonRdevBody",
                              "Your main Shortcut Engine is not rdev, but enabling this feature starts an extra rdev listener alongside it. Turn this feature off to stop this extra rdev usage."
                            )}
                      </p>
                      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
                        <p className="text-red-100/80">
                          {t(
                            "textReplacement.decapitalizeAfterEditRdevWarningLinkHint",
                            "Open Debug -> Experimental Features -> Shortcut Engine and read the rdev warning before using this."
                          )}
                        </p>
                        <button
                          type="button"
                          onClick={() => setSection("debug")}
                          className="font-medium text-red-200 underline decoration-red-300/70 underline-offset-2 transition-colors hover:text-red-100"
                        >
                          {t(
                            "textReplacement.decapitalizeAfterEditRdevWarningLink",
                            "Open Debug Shortcut Settings"
                          )}
                        </button>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            )}

            <div
              id={getShortcutAnchorId(
                "text_replacement_decapitalize_after_edit_key",
              )}
              tabIndex={-1}
              className="px-4 py-3 border-t border-white/[0.05]"
            >
              <div className="space-y-2">
                <div className="text-sm font-medium text-[#f5f5f5]">
                  {t("textReplacement.decapitalizeAfterEditKeyLabel", "Monitored Key")}
                </div>
                <div className="text-xs text-[#b8b8b8]">
                  {t(
                    "textReplacement.decapitalizeAfterEditKeyDescription",
                    "Click the field and press any key or key combination. Monitoring is passive, and any combo containing the monitored key also counts."
                  )}
                </div>
                <div className="text-xs text-[#f2c97b]">
                  {t(
                    "textReplacement.decapitalizeAfterEditComboHint",
                    "If you choose Backspace, Ctrl+Backspace also triggers."
                  )}
                </div>
                <HotkeyCapture
                  value={decapitalizeAfterEditKey}
                  isCapturing={capturingDecapTarget === "primary"}
                  onStartCapture={() => {
                    setDecapCaptureError(null);
                    setCapturingDecapTarget("primary");
                  }}
                  onCaptured={(hotkey) =>
                    saveDecapMonitoredKey(
                      "primary",
                      "text_replacement_decapitalize_after_edit_key",
                      hotkey,
                    )
                  }
                  onCancel={() => setCapturingDecapTarget(null)}
                  disabled={isUpdating("text_replacement_decapitalize_after_edit_key")}
                  osType={hotkeyOsType}
                />
                {decapCaptureError?.target === "primary" && (
                  <div
                    role="alert"
                    aria-live="assertive"
                    className="mt-2 text-xs text-red-200"
                  >
                    {decapCaptureError.message}
                  </div>
                )}

                <div className="mt-3 rounded-md border border-white/[0.08] bg-white/[0.02] px-3 py-3">
                  <ToggleSwitch
                    checked={decapitalizeAfterEditSecondaryKeyEnabled}
                    onChange={(enabled) =>
                      (updateSetting as any)(
                        "text_replacement_decapitalize_after_edit_secondary_key_enabled",
                        enabled
                      )
                    }
                    isUpdating={isUpdating(
                      "text_replacement_decapitalize_after_edit_secondary_key_enabled"
                    )}
                    label={t(
                      "textReplacement.decapitalizeAfterEditSecondaryKeyEnableLabel",
                      "Enable Secondary Monitored Key (OR)"
                    )}
                    description={t(
                      "textReplacement.decapitalizeAfterEditSecondaryKeyEnableDescription",
                      "Default is OFF. When enabled, pressing either primary or secondary key triggers decapitalize monitoring."
                    )}
                    descriptionMode="inline"
                  />

                  {decapitalizeAfterEditSecondaryKeyEnabled && (
                    <div
                      id={getShortcutAnchorId(
                        "text_replacement_decapitalize_after_edit_secondary_key",
                      )}
                      tabIndex={-1}
                      className="mt-3 space-y-2"
                    >
                      <div className="text-xs text-[#b8b8b8]">
                        {t(
                          "textReplacement.decapitalizeAfterEditSecondaryKeyDescription",
                          "Optional second key. Works with OR logic together with the primary key."
                        )}
                      </div>
                      <HotkeyCapture
                        value={decapitalizeAfterEditSecondaryKey}
                        isCapturing={capturingDecapTarget === "secondary"}
                        onStartCapture={() => {
                          setDecapCaptureError(null);
                          setCapturingDecapTarget("secondary");
                        }}
                        onCaptured={(hotkey) =>
                          saveDecapMonitoredKey(
                            "secondary",
                            "text_replacement_decapitalize_after_edit_secondary_key",
                            hotkey,
                          )
                        }
                        onCancel={() => setCapturingDecapTarget(null)}
                        disabled={isUpdating("text_replacement_decapitalize_after_edit_secondary_key")}
                        osType={hotkeyOsType}
                      />
                      {decapCaptureError?.target === "secondary" && (
                        <div
                          role="alert"
                          aria-live="assertive"
                          className="text-xs text-red-200"
                        >
                          {decapCaptureError.message}
                        </div>
                      )}
                    </div>
                  )}
                </div>

                {decapConflicts.length > 0 && (
                  <div className="rounded-md border border-[#7a5d2a]/80 bg-[#4a3a1c]/40 px-3 py-2 text-xs text-[#f2d8a6]">
                    <div className="font-medium">
                      {t(
                        "textReplacement.decapitalizeAfterEditConflictWarningTitle",
                        "Potential overlap with other shortcuts:"
                      )}
                    </div>
                    <div className="mt-1">
                      {t(
                        "textReplacement.decapitalizeAfterEditConflictWarningDescription",
                        "Decap monitor may also trigger when these shortcuts are pressed."
                      )}
                    </div>
                    <ul className="mt-1 list-disc list-inside space-y-0.5">
                      {decapConflicts.map((item) => (
                        <li key={item.id}>
                          {item.name}: {formatKeyCombination(item.binding, hotkeyOsType)}
                        </li>
                      ))}
                    </ul>
                  </div>
                )}
              </div>
            </div>

            <div className="px-4 py-3 border-t border-white/[0.05]">
              <Slider
                value={decapitalizeAfterEditTimeoutMs}
                onChange={(value) =>
                  updateSetting(
                    "text_replacement_decapitalize_timeout_ms",
                    Math.round(value)
                  )
                }
                min={100}
                max={60000}
                step={100}
                label={t(
                  "textReplacement.decapitalizeAfterEditTimeoutLabel",
                  "Trigger Timeout"
                )}
                description={t(
                  "textReplacement.decapitalizeAfterEditTimeoutDescription",
                  "How long (in milliseconds) the decapitalize trigger stays active after pressing the monitored key."
                )}
                descriptionMode="inline"
                formatValue={(value) => `${Math.round(value)} ms`}
              />
            </div>

            <div className="px-4 py-3 border-t border-white/[0.05]">
              <Slider
                value={decapitalizeStandardPostRecordingMonitorMs}
                onChange={(value) =>
                  updateSetting(
                    "text_replacement_decapitalize_standard_post_recording_monitor_ms",
                    Math.round(value)
                  )
                }
                min={0}
                max={60000}
                step={100}
                label={t(
                  "textReplacement.decapitalizeAfterEditStandardPostMonitorLabel",
                  "Standard STT Post-stop Monitor Window"
                )}
                description={t(
                  "textReplacement.decapitalizeAfterEditStandardPostMonitorDescription",
                  "After stopping a standard (non-realtime) recording, keep listening for the monitored key for this many milliseconds. 0 disables this extra window."
                )}
                descriptionMode="inline"
                formatValue={(value) => `${Math.round(value)} ms`}
              />
            </div>

            <div className="px-4 py-3 border-t border-white/[0.05]">
              <TellMeMore
                title={t(
                  "textReplacement.decapitalizeAfterEditTellMeMoreTitle",
                  "Tell me more: Decapitalize After Manual Edit"
                )}
              >
                <div className="space-y-2 text-sm">
                  <p className="text-[#b8b8b8]">
                    {t(
                      "textReplacement.decapitalizeAfterEditDescription",
                      "Passively monitor one key (default: Backspace). After pressing it, the next suitable transcription chunk can start with lowercase to avoid unwanted sentence capitalization."
                    )}
                  </p>
                  <p className="text-[#f5f5f5] font-medium">
                    {t("textReplacement.decapitalizeAfterEditHowItWorksTitle", "How it works")}
                  </p>
                  <ul className="list-disc list-inside space-y-1 text-[#b8b8b8]">
                    <li>
                      <strong>{t("textReplacement.decapitalizeAfterEditStandardModeTitle", "Standard STT:")}</strong>{" "}
                      {t(
                        "textReplacement.decapitalizeAfterEditStandardModeDesc",
                        "after stop-recording in standard STT, AivoRelay can keep monitoring for the configured post-stop window. If you press the monitored key there, the next matching uppercase-start output is decapitalized."
                      )}
                    </li>
                    <li>
                      <strong>{t("textReplacement.decapitalizeAfterEditRealtimeModeTitle", "Realtime API:")}</strong>{" "}
                      {t(
                        "textReplacement.decapitalizeAfterEditRealtimeModeDesc",
                        "while live chunk typing is running, the next incoming matching chunk that starts uppercase is decapitalized immediately, then the trigger is consumed."
                      )}
                    </li>
                  </ul>
                  <p className="text-[#b8b8b8]">
                    {t(
                      "textReplacement.decapitalizeAfterEditLanguageNote",
                      "If the language/script has no uppercase/lowercase distinction, nothing is changed."
                    )}
                  </p>
                  <p className="text-[#f5f5f5] font-medium">
                    {t("textReplacement.decapitalizeAfterEditExampleTitle", "Example")}
                  </p>
                  <ol className="list-decimal list-inside space-y-1 text-[#b8b8b8]">
                    <li>{t("textReplacement.decapitalizeAfterEditExample1", "You dictated: This is knight.")}</li>
                    <li>{t("textReplacement.decapitalizeAfterEditExample2", "You delete the last word with Backspace.")}</li>
                    <li>{t("textReplacement.decapitalizeAfterEditExample3", "You dictate again: night.")}</li>
                    <li>{t("textReplacement.decapitalizeAfterEditExample4", "Result becomes: This is night. (not This is Night.)")}</li>
                  </ol>
                </div>
              </TellMeMore>
            </div>
          </>
        )}
      </SettingsGroup>

      {/* Main Settings Group */}
      <SettingsGroup
        title={t("textReplacement.title", "Text Processing")}
        description={t(
          "textReplacement.description",
          "Automatically replace text patterns in transcriptions. Useful for fixing commonly misheard words or applying consistent formatting."
        )}
      >
        {/* Enable Toggle */}
        <div className="px-4 py-3">
          <ToggleSwitch
            checked={isEnabled}
            onChange={(enabled) =>
              updateSetting("text_replacements_enabled", enabled)
            }
            isUpdating={isUpdating("text_replacements_enabled")}
            label={t("textReplacement.enable", "Enable Text Replacement")}
            description={t(
              "textReplacement.enableDescription",
              "Apply replacement rules to all transcriptions after processing."
            )}
            descriptionMode="inline"
          />
        </div>

        {/* Apply Before LLM Toggle - only show when post-processing is enabled */}
        {getActiveProfilePostProcessingEnabled(settings) && (
          <div className="px-4 py-3 border-t border-white/[0.05]">
            <ToggleSwitch
              checked={settings?.text_replacements_before_llm ?? false}
              onChange={(enabled) =>
                updateSetting("text_replacements_before_llm", enabled)
              }
              isUpdating={isUpdating("text_replacements_before_llm")}
              label={t("textReplacement.beforeLlm", "Apply Before LLM Post-Processing")}
              description={t(
                "textReplacement.beforeLlmDescription",
                "When enabled, text replacements are applied BEFORE LLM processing. This prevents the LLM from modifying your replacement patterns."
              )}
              descriptionMode="inline"
            />
          </div>
        )}

        {/* Help Section */}
        <div className="px-4 py-3 border-t border-white/[0.05]">
          <button
            onClick={() => setShowHelp(!showHelp)}
            className="flex items-center gap-2 text-sm text-[#9b5de5] hover:text-[#b47eff] transition-colors"
          >
            <HelpCircle className="w-4 h-4" />
            {t(
              "textReplacement.helpTitle",
              "I want to use special characters in my replacement",
            )}
            {showHelp ? (
              <ChevronUp className="w-4 h-4" />
            ) : (
              <ChevronDown className="w-4 h-4" />
            )}
          </button>

          {showHelp && (
            <div className="mt-3 p-4 bg-[#1a1a1a] rounded-lg border border-[#333333] text-sm">
              <h4 className="font-medium text-[#f5f5f5] mb-2">
                {t(
                  "textReplacement.escapeSequences",
                  "Special characters in replacements",
                )}
              </h4>
              <p className="text-[#b8b8b8] mb-3">
                {t(
                  "textReplacement.escapeIntro",
                  "The computer cannot tell which characters are instructions for the program and which characters you want to use in your replacement. Put a backslash (\\) before them so the program knows what you mean. This technique is called \"Escapement\". Tip: you can ask your AI for help with it."
                )}
              </p>
              <ul className="space-y-2 text-[#b8b8b8]">
                <li className="flex items-center gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5]">
                    \n
                  </code>
                  <span>→</span>
                  <span>
                    {t(
                      "textReplacement.escapeNewline",
                      "Line break (LF - Unix/Mac style)"
                    )}
                  </span>
                </li>
                <li className="flex items-center gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5]">
                    \r\n
                  </code>
                  <span>→</span>
                  <span>
                    {t(
                      "textReplacement.escapeCRLF",
                      "Line break (CRLF - Windows style)"
                    )}
                  </span>
                </li>
                <li className="flex items-center gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5]">
                    \r
                  </code>
                  <span>→</span>
                  <span>
                    {t(
                      "textReplacement.escapeCarriageReturn",
                      "Carriage return (CR - old Mac style)"
                    )}
                  </span>
                </li>
                <li className="flex items-center gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5]">
                    \t
                  </code>
                  <span>→</span>
                  <span>{t("textReplacement.escapeTab", "Tab character")}</span>
                </li>
                <li className="flex items-center gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5]">
                    \\
                  </code>
                  <span>→</span>
                  <span>
                    {t("textReplacement.escapeBackslash", "Literal backslash")}
                  </span>
                </li>
                <li className="flex items-center gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5]">
                    {"\\u{200D}"}
                  </code>
                  <span>→</span>
                  <span>
                    {t("textReplacement.escapeUnicode", "Unicode character (e.g., \\u{200D} for Zero Width Joiner)")}
                  </span>
                </li>
              </ul>

              <h4 className="font-medium text-[#f5f5f5] mt-4 mb-2">
                {t("textReplacement.optionsTitle", "Options")}
              </h4>
              <ul className="space-y-2 text-[#b8b8b8]">
                <li className="flex items-start gap-2">
                  <CaseSensitive className="w-4 h-4 mt-0.5 text-[#9b5de5] shrink-0" />
                  <span>
                    <strong>{t("textReplacement.caseSensitiveTitle", "Case Sensitive")}</strong> — {t("textReplacement.caseSensitiveDesc", "When enabled, 'Hello' and 'hello' are treated as different. When disabled, both will match.")}
                  </span>
                </li>
                <li className="flex items-start gap-2">
                  <Regex className="w-4 h-4 mt-0.5 text-[#f97316] shrink-0" />
                  <span>
                    <strong>{t("textReplacement.regexTitle", "Regular Expression")}</strong> — {t("textReplacement.regexDesc", "Enable to use regex patterns for advanced matching. Use $1, $2 in replacement for capture groups.")}
                  </span>
                </li>
              </ul>

              <h4 className="font-medium text-[#f5f5f5] mt-4 mb-2">
                {t("textReplacement.examples", "Examples")}
              </h4>
              <ul className="space-y-2 text-[#b8b8b8]">
                <li>
                  <code className="text-[#808080]">teh</code> →{" "}
                  <code className="text-[#4ade80]">the</code>
                  <span className="text-[#606060] ml-2">
                    {t("textReplacement.exampleTypo", "(fix typo)")}
                  </span>
                </li>
                <li>
                  <code className="text-[#808080]">.\n</code> →{" "}
                  <code className="text-[#4ade80]">.\n\n</code>
                  <span className="text-[#606060] ml-2">
                    {t(
                      "textReplacement.exampleParagraph",
                      "(double-space after periods)"
                    )}
                  </span>
                </li>
                <li>
                  <code className="text-[#f97316]">{String.raw`(\d{4})-(\d{2})-(\d{2})`}</code> →{" "}
                  <code className="text-[#4ade80]">$3.$2.$1</code>
                  <span className="text-[#606060] ml-2">
                    {t("textReplacement.exampleRegex", "(reformat a date)")}
                  </span>
                </li>
              </ul>

              <div className="mt-4 p-3 bg-[#252525] rounded border border-[#444444]">
                <p className="text-[#b8b8b8] text-xs">
                  <strong className="text-[#f5f5f5]">
                    {t("textReplacement.noteTitle", "Note:")}
                  </strong>{" "}
                  {t(
                    "textReplacement.noteContent",
                    "For Windows line endings conversion, consider using the 'Convert LF to CRLF' option in Advanced settings instead — it handles this automatically for clipboard paste operations."
                  )}
                </p>
              </div>
            </div>
          )}
        </div>

        {/* Rule transfer */}
        <div className="border-y border-white/[0.05] px-4 py-3">
          <h4 className="mb-3 text-sm font-medium text-[#d0d0d0]">
            {t(
              "textReplacement.transferRulesTitle",
              "Import or export replacement rules",
            )}
          </h4>
          <div className="flex flex-wrap items-center gap-2">
            <Button
              type="button"
              variant="secondary"
              size="sm"
              onClick={() => void handleImportRules()}
              disabled={
                ruleTransferBusy !== null ||
                pendingImportRules !== null ||
                isUpdating("text_replacements")
              }
              title={t("textReplacement.importJson", "Import JSON")}
              className="inline-flex min-w-0 items-center justify-center gap-1.5 whitespace-nowrap focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#9b5de5]/60"
            >
              {ruleTransferBusy === "import" ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" aria-hidden="true" />
              ) : (
                <Upload className="h-3.5 w-3.5" aria-hidden="true" />
              )}
              {t("textReplacement.importJson", "Import JSON")}
            </Button>
            <Button
              type="button"
              variant="secondary"
              size="sm"
              onClick={() => void handleExportRules()}
              disabled={
                replacements.length === 0 ||
                ruleTransferBusy !== null ||
                pendingImportRules !== null ||
                isUpdating("text_replacements")
              }
              title={t("textReplacement.exportJson", "Export JSON")}
              className="inline-flex min-w-0 items-center justify-center gap-1.5 whitespace-nowrap focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#9b5de5]/60"
            >
              {ruleTransferBusy === "export" ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" aria-hidden="true" />
              ) : (
                <Download className="h-3.5 w-3.5" aria-hidden="true" />
              )}
              {t("textReplacement.exportJson", "Export JSON")}
            </Button>
          </div>
        </div>

        {/* Add New Rule */}
        <div className="overflow-hidden px-4 py-4">
          <h4 className="mb-3 text-sm font-medium text-[#d0d0d0]">
            {t(
              "textReplacement.addRuleTitle",
              "Add a new replacement rule",
            )}
          </h4>
          <div className="mb-2 flex w-full items-center gap-2">
            <div className="min-w-0 flex-1">
              <Input
                type="text"
                className="w-full"
                value={newFrom}
                onChange={(e) => setNewFrom(e.target.value)}
                onKeyDown={handleKeyPress}
                placeholder={t("textReplacement.fromPlaceholder", "Find text...")}
                variant="compact"
                disabled={isUpdating("text_replacements")}
              />
            </div>
            <ArrowRight className="h-4 w-4 shrink-0 text-[#606060]" />
            <div className="min-w-0 flex-1">
              <Input
                type="text"
                className="w-full"
                value={newTo}
                onChange={(e) => setNewTo(e.target.value)}
                onKeyDown={handleKeyPress}
                placeholder={t(
                  "textReplacement.toPlaceholder",
                  "Replace with...",
                )}
                variant="compact"
                disabled={isUpdating("text_replacements")}
              />
            </div>
          </div>
          {/* Options row */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <button
                onClick={() => setNewCaseSensitive(!newCaseSensitive)}
                className={`flex items-center gap-1.5 px-2 py-1 rounded text-xs transition-colors ${
                  newCaseSensitive
                    ? "bg-[#9b5de5]/20 text-[#9b5de5] border border-[#9b5de5]/30"
                    : "bg-[#252525] text-[#606060] border border-[#333333]"
                }`}
                title={t("textReplacement.caseSensitiveTooltip", "Toggle case sensitivity")}
              >
                <CaseSensitive className="w-3.5 h-3.5" />
                {t("textReplacement.caseSensitiveShort", "Aa")}
              </button>
              <button
                onClick={() => setNewIsRegex(!newIsRegex)}
                className={`flex items-center gap-1.5 px-2 py-1 rounded text-xs transition-colors ${
                  newIsRegex
                    ? "bg-[#f97316]/20 text-[#f97316] border border-[#f97316]/30"
                    : "bg-[#252525] text-[#606060] border border-[#333333]"
                }`}
                title={t("textReplacement.regexTooltip", "Toggle regex mode")}
              >
                <Regex className="w-3.5 h-3.5" />
                {t("textReplacement.regexShort", ".*")}
              </button>
            </div>
            <Button
              onClick={handleAddRule}
              disabled={newFrom.length === 0 || isUpdating("text_replacements")}
              variant="primary"
              size="md"
              className="shrink-0"
            >
              <Plus className="w-4 h-4" />
            </Button>
          </div>
        </div>

        {/* Rule search and display order */}
        <div
          className="space-y-2 px-4 py-3"
          style={{ borderTopWidth: 0 }}
        >
          <div className="relative">
            <Search
              className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-[#606060]"
              aria-hidden="true"
            />
            <Input
              type="text"
              variant="compact"
              value={ruleSearchQuery}
              onChange={(event) => setRuleSearchQuery(event.target.value)}
              placeholder={t(
                "textReplacement.filterPlaceholder",
                "Filter replacement rules...",
              )}
              aria-label={t(
                "textReplacement.filterPlaceholder",
                "Filter replacement rules...",
              )}
              className="w-full pl-8 pr-8"
            />
            {ruleSearchQuery.length > 0 && (
              <button
                type="button"
                onClick={() => setRuleSearchQuery("")}
                aria-label={t(
                  "textReplacement.clearFilter",
                  "Clear rule filter",
                )}
                title={t(
                  "textReplacement.clearFilter",
                  "Clear rule filter",
                )}
                className="absolute right-2 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded text-[#606060] transition-colors hover:bg-white/[0.06] hover:text-[#d0d0d0]"
              >
                <X className="h-3.5 w-3.5" aria-hidden="true" />
              </button>
            )}
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <div
              role="group"
              aria-label={t(
                "textReplacement.filterScope",
                "Rule filter scope",
              )}
              className="inline-flex overflow-hidden rounded-md border border-[#3c3c3c] bg-[#181818]"
            >
              <button
                type="button"
                aria-pressed={ruleSearchScope === "all"}
                onClick={() => setRuleSearchScope("all")}
                className={`min-h-7 whitespace-nowrap px-2.5 py-1 text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[#9b5de5]/60 ${
                  ruleSearchScope === "all"
                    ? "bg-[#9b5de5]/20 text-[#c69cff]"
                    : "text-[#8a8a8a] hover:bg-white/[0.04] hover:text-[#c8c8c8]"
                }`}
              >
                {t("textReplacement.filterAllFields", "All fields")}
              </button>
              <button
                type="button"
                aria-pressed={ruleSearchScope === "replacement"}
                onClick={() => setRuleSearchScope("replacement")}
                className={`min-h-7 whitespace-nowrap border-l border-[#3c3c3c] px-2.5 py-1 text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[#9b5de5]/60 ${
                  ruleSearchScope === "replacement"
                    ? "bg-[#9b5de5]/20 text-[#c69cff]"
                    : "text-[#8a8a8a] hover:bg-white/[0.04] hover:text-[#c8c8c8]"
                }`}
              >
                {t("textReplacement.filterReplaceWith", "Replace with")}
              </button>
            </div>

            <span
              className="text-xs tabular-nums text-[#707070]"
              aria-live="polite"
            >
              {filteredReplacementCount > 0
                ? t(
                    "textReplacement.filterCountFiltered",
                    "Showing rules: {{visible}} out of {{total}}; {{filtered}} filtered by search",
                    {
                      visible: visibleReplacements.length,
                      total: replacements.length,
                      filtered: filteredReplacementCount,
                    },
                  )
                : t(
                    "textReplacement.filterCount",
                    "Showing rules: {{visible}} out of {{total}}",
                    {
                      visible: visibleReplacements.length,
                      total: replacements.length,
                    },
                  )}
            </span>

          </div>
        </div>

        {/* Rules List */}
        {replacements.length > 0 && (
          <div className="px-4 py-3" style={{ borderTopWidth: 0 }}>
            <div className="mb-2 flex items-center gap-3 px-3">
              <span className="h-4 w-4 shrink-0" aria-hidden="true" />

              <ColumnSortControl
                column="find"
                direction={findSortDirection}
                onChange={(direction) => setColumnSort("find", direction)}
                onTurnOff={() => setRuleSortOrder("added")}
              />

              <span className="h-4 w-4 shrink-0" aria-hidden="true" />

              <ColumnSortControl
                column="replacement"
                direction={replacementSortDirection}
                onChange={(direction) =>
                  setColumnSort("replacement", direction)
                }
                onTurnOff={() => setRuleSortOrder("added")}
              />

              <span className="h-7 w-10 shrink-0" aria-hidden="true" />
            </div>

            <div className="space-y-2">
              {visibleReplacements.map((rule) => (
                <div
                  key={rule.id}
                  className={`p-3 rounded-lg border transition-all ${
                    rule.enabled
                      ? "bg-[#1a1a1a] border-[#333333]"
                      : "bg-[#0f0f0f] border-[#252525] opacity-60"
                  }`}
                >
                  {/* Main row */}
                  <div className="flex items-center gap-3">
                    {/* Enable/Disable Checkbox */}
                    <input
                      type="checkbox"
                      checked={rule.enabled}
                      onChange={() => handleToggleRule(rule.id)}
                      className="accent-[#9b5de5] w-4 h-4 rounded shrink-0"
                      disabled={isUpdating("text_replacements")}
                    />

                    {editingId === rule.id ? (
                      /* Edit mode */
                      <>
                        <div className="flex-1 min-w-0">
                          <Input
                            type="text"
                            className="w-full"
                            value={editFrom}
                            onChange={(e) => setEditFrom(e.target.value)}
                            onKeyDown={handleEditKeyPress}
                            variant="compact"
                            autoFocus
                          />
                        </div>
                        <ArrowRight className="w-4 h-4 text-[#606060] shrink-0" />
                        <div className="flex-1 min-w-0">
                          <Input
                            type="text"
                            className="w-full"
                            value={editTo}
                            onChange={(e) => setEditTo(e.target.value)}
                            onKeyDown={handleEditKeyPress}
                            variant="compact"
                          />
                        </div>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={saveEditing}
                          className="shrink-0 text-[#4ade80] hover:text-[#22c55e]"
                          title={t("textReplacement.save", "Save")}
                        >
                          <Check className="w-4 h-4" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={cancelEditing}
                          className="shrink-0 text-[#808080] hover:text-red-400"
                          title={t("textReplacement.cancel", "Cancel")}
                        >
                          <X className="w-4 h-4" />
                        </Button>
                      </>
                    ) : (
                      /* View mode */
                      <>
                        {/* From - clickable to edit */}
                        <div 
                          className="flex-1 min-w-0 cursor-pointer"
                          onClick={() => startEditing(rule)}
                          title={t("textReplacement.clickToEdit", "Click to edit")}
                        >
                          <code
                            className={`text-sm px-2 py-1 rounded block truncate hover:ring-1 hover:ring-[#9b5de5]/50 ${
                              rule.enabled
                                ? "bg-[#252525] text-[#f5f5f5]"
                                : "bg-[#1a1a1a] text-[#808080]"
                            } ${rule.is_regex ? "border-l-2 border-[#f97316]" : ""}`}
                          >
                            {formatDisplayText(rule.from)}
                          </code>
                        </div>

                        {/* Arrow */}
                        <ArrowRight
                          className={`w-4 h-4 shrink-0 ${
                            rule.enabled ? "text-[#9b5de5]" : "text-[#444444]"
                          }`}
                        />

                        {/* To - clickable to edit */}
                        <div 
                          className="flex-1 min-w-0 cursor-pointer"
                          onClick={() => startEditing(rule)}
                          title={t("textReplacement.clickToEdit", "Click to edit")}
                        >
                          <code
                            className={`text-sm px-2 py-1 rounded block truncate hover:ring-1 hover:ring-[#9b5de5]/50 ${
                              rule.enabled
                                ? "bg-[#252525] text-[#4ade80]"
                                : "bg-[#1a1a1a] text-[#606060]"
                            }`}
                          >
                            {formatDisplayText(rule.to)}
                          </code>
                        </div>

                        {/* Delete Button */}
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => handleRemoveRule(rule.id)}
                          disabled={isUpdating("text_replacements")}
                          className="shrink-0 text-[#808080] hover:text-red-400"
                          title={t("textReplacement.delete", "Delete rule")}
                        >
                          <Trash2 className="w-4 h-4" />
                        </Button>
                      </>
                    )}
                  </div>

                  {/* Options row - only show when not editing */}
                  {editingId !== rule.id && (
                    <div className="flex items-center gap-2 mt-2 ml-7">
                      <button
                        onClick={() => handleToggleCaseSensitive(rule.id)}
                        disabled={isUpdating("text_replacements")}
                        className={`flex items-center gap-1 px-1.5 py-0.5 rounded text-xs transition-colors ${
                          rule.case_sensitive
                            ? "bg-[#9b5de5]/20 text-[#9b5de5]"
                            : "bg-[#252525] text-[#606060]"
                        }`}
                        title={t("textReplacement.caseSensitiveTooltip", "Toggle case sensitivity")}
                      >
                        <CaseSensitive className="w-3 h-3" />
                      </button>
                      <button
                        onClick={() => handleToggleIsRegex(rule.id)}
                        disabled={isUpdating("text_replacements")}
                        className={`flex items-center gap-1 px-1.5 py-0.5 rounded text-xs transition-colors ${
                          rule.is_regex
                            ? "bg-[#f97316]/20 text-[#f97316]"
                            : "bg-[#252525] text-[#606060]"
                        }`}
                        title={t("textReplacement.regexTooltip", "Toggle regex mode")}
                      >
                        <Regex className="w-3 h-3" />
                      </button>
                    </div>
                  )}
                </div>
              ))}
              {visibleReplacements.length === 0 && (
                <div className="rounded-md border border-dashed border-[#333333] px-4 py-6 text-center text-sm text-[#707070]">
                  {t(
                    "textReplacement.noFilterResults",
                    "No replacement rules match this filter.",
                  )}
                </div>
              )}
            </div>
          </div>
        )}

        {/* Empty State */}
        {replacements.length === 0 && (
          <div className="px-4 py-6 text-center text-[#606060]">
            <p className="text-sm">
              {t(
                "textReplacement.empty",
                "No replacement rules yet. Add one above to get started."
              )}
            </p>
          </div>
        )}
      </SettingsGroup>

      {/* Speech Clean-up Group */}
      <SettingsGroup
        title={t("textReplacement.cleanupTitle", "Speech Clean-up")}
        description={t(
          "textReplacement.cleanupDescription",
          "Automatically remove common speech artifacts from the final text."
        )}
      >
        {/* Filler Word Filter */}
        <div className="px-4 py-3">
          <ToggleSwitch
            checked={settings?.filler_word_filter_enabled ?? false}
            onChange={(enabled) =>
              updateSetting("filler_word_filter_enabled", enabled)
            }
            isUpdating={isUpdating("filler_word_filter_enabled")}
            label={t("audioProcessing.fillerFilter", "Remove Filler Words")}
            description={t(
              "audioProcessing.fillerFilterDescription",
              "Automatically remove 'uh', 'um', 'hmm' and similar filler words from transcriptions."
            )}
            descriptionMode="inline"
          />
        </div>

        <div className="px-4 pt-3 border-t border-white/[0.05]">
          <TellMeMore
            title={t(
              "audioProcessing.fillerHelpTitle",
              "Tell me more about filler word removal",
            )}
          >
            <div className="text-sm">
              <h4 className="font-medium text-[#f5f5f5] mb-2">
                {t("audioProcessing.whatItDoes", "What it does")}
              </h4>
              <p className="text-[#b8b8b8] mb-3">
                {t(
                  "audioProcessing.fillerExplanation",
                  "This feature automatically removes common filler words and speech artifacts from your transcriptions:"
                )}
              </p>
              <ul className="space-y-1 text-[#b8b8b8] mb-3">
                <li>• <strong>{t("audioProcessing.fillerWords", "Filler words:")}</strong> uh, um, uhm, umm, ah, eh, hmm, hm, mmm</li>
                <li>• <strong>{t("audioProcessing.hallucinations", "Hallucinations:")}</strong> [AUDIO], (pause), {"<tag>...</tag>"}</li>
                <li>• <strong>{t("audioProcessing.stutters", "Stutters:")}</strong> "w wh wh wh why" → "wh why"</li>
              </ul>

              <h4 className="font-medium text-[#f5f5f5] mt-4 mb-2">
                {t("audioProcessing.howItWorksTitle", "How it works (technical)")}
              </h4>
              <p className="text-[#b8b8b8] mb-2">
                {t(
                  "audioProcessing.howItWorksIntro",
                  "The filter applies several regex patterns in sequence:"
                )}
              </p>
              <ul className="space-y-2 text-[#b8b8b8] mb-3">
                <li className="flex items-start gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5] text-xs whitespace-nowrap shrink-0">{"<TAG>...</TAG>"}</code>
                  <span>→ {t("audioProcessing.regexTagBlock", "Removes XML-style tag blocks (model hallucinations)")}</span>
                </li>
                <li className="flex items-start gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5] text-xs whitespace-nowrap shrink-0">[...]  (...)  {"{"}"...{"}"}</code>
                  <span>→ {t("audioProcessing.regexBrackets", "Removes bracketed content like [AUDIO], (pause), {noise}")}</span>
                </li>
                <li className="flex items-start gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5] text-xs whitespace-nowrap shrink-0">\\b(uh|um|...)\\b</code>
                  <span>→ {t("audioProcessing.regexFillers", "Removes filler words with word boundaries")}</span>
                </li>
                <li className="flex items-start gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5] text-xs whitespace-nowrap shrink-0">{t("audioProcessing.regexStutterPattern", "3+ repetitions")}</code>
                  <span>→ {t("audioProcessing.regexStutters", "Collapses repeated 1-2 letter words (I I I I → I)")}</span>
                </li>
              </ul>

              <div className="mt-4 p-3 bg-[#2a2010] rounded border border-[#f97316]/30">
                <p className="text-[#f5f5f5] text-xs">
                  <strong className="text-[#f97316]">
                    {t("audioProcessing.languageWarningTitle", "⚠️ Non-English Languages:")}
                  </strong>{" "}
                  {t(
                    "audioProcessing.languageWarning",
                    "This feature is optimized for English. If you experience issues with other languages (missing words, incorrect filtering), try disabling this option."
                  )}
                </p>
              </div>
            </div>
          </TellMeMore>
        </div>

        {/* Zero-Width Character Filter */}
        <div className="px-4 py-3 border-t border-white/[0.05]">
          <ToggleSwitch
            checked={settings?.zero_width_filter_enabled ?? true}
            onChange={(enabled) =>
              updateSetting("zero_width_filter_enabled", enabled)
            }
            isUpdating={isUpdating("zero_width_filter_enabled")}
            label={t("audioProcessing.zeroWidthFilter", "Strip Invisible Characters")}
            description={t(
              "audioProcessing.zeroWidthFilterDescription",
              "Remove invisible Unicode characters from LLM output (post-processing and AI Replace)."
            )}
            descriptionMode="inline"
          />
        </div>

        <div className="px-4 pt-3 border-t border-white/[0.05]">
          <TellMeMore
            title={t(
              "audioProcessing.zeroWidthHelpTitle",
              "Tell me more about invisible character removal",
            )}
          >
            <div className="text-sm">
              <p className="text-[#b8b8b8] mb-3">
                {t(
                  "audioProcessing.zeroWidthExplanation",
                  "Some LLM providers (notably Qwen) insert invisible Unicode characters into their responses. These are zero-width characters that you can't see, but they can cause issues when pasted into other applications."
                )}
              </p>
              <h4 className="font-medium text-[#f5f5f5] mb-2">
                {t("audioProcessing.zeroWidthWhatRemoved", "Characters removed:")}
              </h4>
              <ul className="space-y-1 text-[#b8b8b8] mb-3">
                <li className="flex items-start gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5] text-xs whitespace-nowrap shrink-0">U+200B</code>
                  <span>{t("audioProcessing.zeroWidthZWS", "Zero-Width Space (U+200B)")}</span>
                </li>
                <li className="flex items-start gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5] text-xs whitespace-nowrap shrink-0">U+200C</code>
                  <span>{t("audioProcessing.zeroWidthZWNJ", "Zero-Width Non-Joiner (U+200C)")}</span>
                </li>
                <li className="flex items-start gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5] text-xs whitespace-nowrap shrink-0">U+200D</code>
                  <span>{t("audioProcessing.zeroWidthZWJ", "Zero-Width Joiner (U+200D)")}</span>
                </li>
                <li className="flex items-start gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5] text-xs whitespace-nowrap shrink-0">U+FEFF</code>
                  <span>{t("audioProcessing.zeroWidthBOM", "Byte Order Mark / Zero-Width No-Break Space (U+FEFF)")}</span>
                </li>
              </ul>
              <p className="text-[#b8b8b8] mb-3">
                {t(
                  "audioProcessing.zeroWidthIssues",
                  "These invisible characters can cause unexpected behavior: broken text searches, incorrect string lengths, copy-paste issues, and invisible formatting problems in documents."
                )}
              </p>
              <div className="mt-3 p-3 bg-[#252525] rounded border border-[#444444]">
                <p className="text-[#b8b8b8] text-xs">
                  <strong className="text-[#f5f5f5]">
                    {t("audioProcessing.noteTitle", "Note:")}
                  </strong>{" "}
                  {t(
                    "audioProcessing.zeroWidthNote",
                    "This filter applies to all LLM output — both post-processing and AI Replace. It's safe to leave enabled for all providers."
                  )}
                </p>
              </div>
            </div>
          </TellMeMore>
        </div>
      </SettingsGroup>

      {/* Fuzzy Word Correction Group */}
      <SettingsGroup
        title={t("textReplacement.fuzzyWordCorrectionTitle", "Fuzzy Word Correction")}
        description={t(
          "textReplacement.fuzzyWordCorrectionDescription",
          "Add words that are often misheard (names, technical terms). The system will automatically correct similar-sounding words."
        )}
      >
        <div className="px-4 py-3 bg-white/[0.02] border-b border-white/[0.05]">
          <TellMeMore title={t("textReplacement.fuzzyHowItWorksTitle", "How Fuzzy Correction Works")}>
            <div className="space-y-3 text-sm">
              <p>
                {t(
                  "textReplacement.fuzzyHowItWorksIntro",
                  "This algorithm fixes misheard words by comparing them to your custom list using two methods:"
                )}
              </p>
              <ul className="list-disc list-inside space-y-2 ml-1 opacity-90">
                <li>
                  <strong>{t("textReplacement.fuzzySoundsLikeTitle", "Sounds Like (Phonetic):")}</strong>{" "}
                  {t(
                    "textReplacement.fuzzySoundsLikeDescription",
                    "It recognizes that \"edge\" and \"etch\" sound similar."
                  )}
                </li>
                <li>
                  <strong>{t("textReplacement.fuzzyLooksLikeTitle", "Looks Like (Levenshtein):")}</strong>{" "}
                  {t(
                    "textReplacement.fuzzyLooksLikeDescription",
                    "It catches typos like \"srart\" instead of \"start\"."
                  )}
                </li>
              </ul>
              <p className="pt-1 text-xs text-text/70 italic">
                {t(
                  "textReplacement.fuzzyTip",
                  "Tip: If it corrects words too aggressively, lower the sensitivity slider below."
                )}
              </p>
            </div>
          </TellMeMore>
        </div>

        <CustomWords descriptionMode="inline" grouped={true} />

        {/* N-gram toggle for multi-word fuzzy correction */}
        <div className="px-4 py-3 border-t border-white/[0.05]">
          <ToggleSwitch
            checked={(settings as any)?.custom_words_ngram_enabled ?? true}
            onChange={(enabled) =>
              (updateSetting as any)("custom_words_ngram_enabled", enabled)
            }
            isUpdating={isUpdating("custom_words_ngram_enabled")}
            label={t(
              "textReplacement.multiWordMatchingLabel",
              "Enable Multi-word Matching (N-grams)"
            )}
            description={t(
              "textReplacement.multiWordMatchingDescription",
              "Match up to 3 spoken tokens as one custom term (example: 'Chat G P T' -> 'ChatGPT'). Disable if corrections are too aggressive."
            )}
            descriptionMode="inline"
            grouped={true}
          />
        </div>
        
        {/* Word Correction Threshold */}
        <div className="px-4 py-3 border-t border-white/[0.05]">
          <Slider
            value={settings?.word_correction_threshold ?? 0.18}
            onChange={(value) => updateSetting("word_correction_threshold", value)}
            min={0.0}
            max={1.0}
            label={t("textReplacement.correctionSensitivityLabel", "Correction Sensitivity")}
            description={t(
              "textReplacement.correctionSensitivityDescription",
              "Threshold for fuzzy match score (0.0 = exact match only, 1.0 = accept any). Default 0.18 means a word must be ~82% similar to be corrected."
            )}
            descriptionMode="inline"
            grouped={true}
          />
        </div>
      </SettingsGroup>

      <SettingsGroup
        title={t("textReplacement.outputWhitespaceTitle", "Output Whitespace")}
        description={t(
          "textReplacement.outputWhitespaceDescription",
          "Configure how leading/trailing spaces are normalized in final transcription output."
        )}
      >
        <div className="px-4 py-3">
          <ToggleSwitch
            checked={leadingWhitespaceMode === "remove_if_present"}
            onChange={(enabled) =>
              setLeadingWhitespaceMode(enabled ? "remove_if_present" : "preserve")
            }
            isUpdating={isUpdating("output_whitespace_leading_mode")}
            label={t(
              "textReplacement.outputWhitespaceLeadingRemoveLabel",
              "Remove leading space if provider returned one"
            )}
            description={t(
              "textReplacement.outputWhitespaceLeadingRemoveDescription",
              "If output starts with whitespace, remove it."
            )}
            descriptionMode="inline"
          />
        </div>
        <div className="px-4 py-3 border-t border-white/[0.05]">
          <ToggleSwitch
            checked={leadingWhitespaceMode === "add_if_missing"}
            onChange={(enabled) =>
              setLeadingWhitespaceMode(enabled ? "add_if_missing" : "preserve")
            }
            isUpdating={isUpdating("output_whitespace_leading_mode")}
            label={t(
              "textReplacement.outputWhitespaceLeadingAddLabel",
              "Add leading space if provider did not return one"
            )}
            description={t(
              "textReplacement.outputWhitespaceLeadingAddDescription",
              "If output starts without whitespace, prefix one space."
            )}
            descriptionMode="inline"
          />
        </div>
        <div className="px-4 py-3 border-t border-white/[0.05]">
          <ToggleSwitch
            checked={trailingWhitespaceMode === "remove_if_present"}
            onChange={(enabled) =>
              setTrailingWhitespaceMode(enabled ? "remove_if_present" : "preserve")
            }
            isUpdating={isUpdating("output_whitespace_trailing_mode")}
            label={t(
              "textReplacement.outputWhitespaceTrailingRemoveLabel",
              "Remove trailing space if provider returned one"
            )}
            description={t(
              "textReplacement.outputWhitespaceTrailingRemoveDescription",
              "If output ends with whitespace, remove it."
            )}
            descriptionMode="inline"
          />
        </div>
        <div className="px-4 py-3 border-t border-white/[0.05]">
          <ToggleSwitch
            checked={trailingWhitespaceMode === "add_if_missing"}
            onChange={(enabled) =>
              setTrailingWhitespaceMode(enabled ? "add_if_missing" : "preserve")
            }
            isUpdating={isUpdating("output_whitespace_trailing_mode")}
            label={t(
              "textReplacement.outputWhitespaceTrailingAddLabel",
              "Add trailing space if provider did not return one"
            )}
            description={t(
              "textReplacement.outputWhitespaceTrailingAddDescription",
              "If output ends without whitespace, append one space."
            )}
            descriptionMode="inline"
          />
        </div>
      </SettingsGroup>

      <SettingsGroup
        title={t(
          "textReplacement.sonioxRealtimeChunkTitle",
          "Soniox Realtime Chunks"
        )}
        description={t(
          "textReplacement.sonioxRealtimeChunkDescription",
          "Controls how Soniox Live chunk text is corrected before insertion. Default: both options are OFF."
        )}
      >
        <div className="px-4 py-3">
          <ToggleSwitch
            checked={settings?.soniox_realtime_fuzzy_correction_enabled ?? false}
            onChange={(enabled) =>
              updateSetting("soniox_realtime_fuzzy_correction_enabled", enabled)
            }
            isUpdating={isUpdating("soniox_realtime_fuzzy_correction_enabled")}
            label={t(
              "textReplacement.sonioxRealtimeChunkFuzzyLabel",
              "Enable Fuzzy Word Correction for Soniox Live Chunks"
            )}
            description={t(
              "textReplacement.sonioxRealtimeChunkFuzzyDescription",
              "Uses typo-tolerant matching from Custom Words on each live chunk. If OFF, chunks skip fuzzy correction but regular Text Replacement rules still run."
            )}
            descriptionMode="inline"
          />
        </div>

        <div className="px-4 py-3 border-t border-white/[0.05]">
          <ToggleSwitch
            checked={settings?.soniox_realtime_keep_safety_buffer_enabled ?? false}
            onChange={(enabled) =>
              updateSetting("soniox_realtime_keep_safety_buffer_enabled", enabled)
            }
            isUpdating={isUpdating("soniox_realtime_keep_safety_buffer_enabled")}
            label={t(
              "textReplacement.sonioxRealtimeChunkSafetyBufferLabel",
              "Keep Safety Buffer for Cross-chunk Matching"
            )}
            description={t(
              "textReplacement.sonioxRealtimeChunkSafetyBufferDescription",
              "Keeps the newest ~3 words briefly so fuzzy correction can match across chunk boundaries. This buffer is used only when fuzzy correction is ON. It delays pasting by about those 3 words, so live output may feel a bit slower (often not noticeable)."
            )}
            descriptionMode="inline"
          />
        </div>
        <div className="px-4 pb-3 text-xs text-white/60">
          {t(
            "textReplacement.sonioxRealtimeChunkBehaviorGuide",
            "For fastest live appearance, keep both OFF. Enable both only when you need better cross-chunk fuzzy correction."
          )}
        </div>
      </SettingsGroup>
    <TextReplacementImportDialog
      isOpen={pendingImportRules !== null}
      importedRuleCount={pendingImportRules?.length ?? 0}
      currentRuleCount={replacements.length}
      mode={importMode}
      overwriteConflicts={overwriteImportConflicts}
      isApplying={ruleTransferBusy === "import"}
      onModeChange={(nextMode) => {
        setImportMode(nextMode);
        if (nextMode === "replace") {
          setOverwriteImportConflicts(false);
        }
      }}
      onOverwriteConflictsChange={setOverwriteImportConflicts}
      onCancel={handleCancelImport}
      onConfirm={() => void handleConfirmImport()}
    />
  </div>
  );
};

export default TextReplacementSettings;
