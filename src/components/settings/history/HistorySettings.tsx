import React, { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { AudioPlayer, AudioPlayerGroup } from "../../ui/AudioPlayer";
import { Button } from "../../ui/Button";
import { Tooltip } from "../../ui/Tooltip";
import { ConfirmationModal } from "../../ui/ConfirmationModal";
import { CopyFeedbackIcon } from "../../ui/CopyFeedbackIcon";
import {
  Star,
  Trash2,
  FileText,
  FolderOpen,
  Wand2,
  AlertTriangle,
  RotateCcw,
  Search,
  X,
} from "lucide-react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { platform } from "@tauri-apps/plugin-os";
import { readFile } from "@tauri-apps/plugin-fs";
import { sessionToast as toast } from "@/lib/sessionToast";
import { commands, type HistoryEntry } from "@/bindings";
import { formatDateTime, formatDate } from "@/utils/dateFormat";
import { HandyShortcut } from "../HandyShortcut";
import { HistoryLimit } from "../HistoryLimit";
import { RecordingRetentionPeriodSelector } from "../RecordingRetentionPeriod";
import { Alert } from "../../ui/Alert";
import { Input } from "../../ui/Input";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { SettingContainer } from "../../ui/SettingContainer";
import { useSettings } from "@/hooks/useSettings";
import { useNavigationStore } from "@/stores/navigationStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { navigateToSettingsAnchor } from "@/lib/anchorNavigation";
import { reconcileHistoryPage } from "./historyPage";
import {
  historyDisplayText,
  matchesHistorySearch,
  rawTranscriptToggle,
  splitByQuery,
} from "./search";

const IS_LINUX = platform() === "linux";
const PAGE_SIZE = 30;
const SEARCH_DEBOUNCE_MS = 250;

const HighlightedText: React.FC<{ text: string; query: string }> = ({
  text,
  query,
}) => (
  <>
    {splitByQuery(text, query).map((segment, index) =>
      segment.match ? (
        <mark key={index} className="bg-logo-primary/40 text-inherit rounded-sm">
          {segment.text}
        </mark>
      ) : (
        segment.text
      ),
    )}
  </>
);

interface HistoryReplayProgress {
  id: number;
  sent_seconds: number;
  duration_seconds: number;
  phase: "connecting" | "sending" | "finalizing" | "processing";
}

interface HistoryRetryState {
  progress: HistoryReplayProgress | null;
  stopping: boolean;
}

type HistoryDeleteFailureReason =
  | "file_in_use"
  | "file_locked"
  | "access_denied"
  | "read_only"
  | "write_protected"
  | "database_error"
  | "io_error";

interface HistoryDeleteFailurePayload {
  reason: HistoryDeleteFailureReason;
  file_name: string | null;
  processes: string[];
  os_error: number | null;
  details: string;
}

interface HistoryDeleteErrorPayload {
  failures: HistoryDeleteFailurePayload[];
  deleted_count: number;
  remaining_count: number;
}

const UNKNOWN_HISTORY_DELETE_FAILURE: HistoryDeleteFailurePayload = {
  reason: "io_error",
  file_name: null,
  processes: [],
  os_error: null,
  details: "History deletion failed",
};

class HistoryDeleteCommandError extends Error {
  readonly payload: HistoryDeleteErrorPayload;

  constructor(payload: HistoryDeleteErrorPayload) {
    super(payload.failures[0]?.details ?? "History deletion failed");
    this.name = "HistoryDeleteCommandError";
    this.payload = payload;
  }
}

const fallbackDeleteFailure = (details: string): HistoryDeleteErrorPayload => ({
  failures: [
    {
      ...UNKNOWN_HISTORY_DELETE_FAILURE,
      details,
    },
  ],
  deleted_count: 0,
  remaining_count: 1,
});

const toHistoryDeleteCommandError = (
  error: unknown,
): HistoryDeleteCommandError => {
  if (error instanceof HistoryDeleteCommandError) {
    return error;
  }

  const details = error instanceof Error ? error.message : String(error);
  try {
    const parsed = JSON.parse(details) as Partial<HistoryDeleteErrorPayload>;
    if (
      Array.isArray(parsed.failures) &&
      typeof parsed.deleted_count === "number" &&
      typeof parsed.remaining_count === "number"
    ) {
      return new HistoryDeleteCommandError(parsed as HistoryDeleteErrorPayload);
    }
  } catch {
    // Older backends and transport failures return plain text.
  }

  return new HistoryDeleteCommandError(fallbackDeleteFailure(details));
};

const deleteFailureDescription = (
  t: TFunction,
  failure: HistoryDeleteFailurePayload,
) => {
  switch (failure.reason) {
    case "file_in_use":
    case "file_locked":
      return failure.processes.length > 0
        ? t("settings.history.deleteFailure.fileInUseProcess", {
            processes: failure.processes.join(", "),
          })
        : t("settings.history.deleteFailure.fileInUse");
    case "access_denied":
      return t("settings.history.deleteFailure.accessDenied");
    case "read_only":
      return t("settings.history.deleteFailure.readOnly");
    case "write_protected":
      return t("settings.history.deleteFailure.writeProtected");
    case "database_error":
      return t("settings.history.deleteFailure.databaseError");
    default:
      return t("settings.history.deleteFailure.ioError");
  }
};

const showHistoryDeleteFailure = (
  t: TFunction,
  error: unknown,
  retry: () => void,
) => {
  const deletionError = toHistoryDeleteCommandError(error);
  const { payload } = deletionError;
  const failure = payload.failures[0] ?? UNKNOWN_HISTORY_DELETE_FAILURE;
  const isPartial = payload.deleted_count > 0;
  const recordsRemain = payload.remaining_count > 0;
  const isPermissionFailure =
    failure.reason === "access_denied" ||
    failure.reason === "read_only" ||
    failure.reason === "write_protected";
  const help = isPermissionFailure
    ? t("settings.history.deleteFailure.permissionHelp")
    : failure.reason === "file_in_use" || failure.reason === "file_locked"
      ? t("settings.history.deleteFailure.fileInUseHelp")
      : t("settings.history.deleteFailure.retryHelp");

  toast.error(
    isPartial
      ? t("settings.history.deleteFailure.partialTitle", {
          deleted: payload.deleted_count,
          remaining: payload.remaining_count,
        })
      : t("settings.history.deleteFailure.title"),
    {
      description: `${deleteFailureDescription(t, failure)} ${help}`,
      duration: 12000,
      action: isPermissionFailure || !recordsRemain
        ? {
            label: t("settings.history.deleteFailure.openFolderAction"),
            onClick: () => {
              void commands.openRecordingsFolder();
            },
          }
        : {
            label: t("settings.history.deleteFailure.retryAction"),
            onClick: retry,
          },
    },
  );
};

interface PaginatedHistory {
  entries: HistoryEntry[];
  has_more: boolean;
}

type HistoryUpdatePayload =
  | { action: "added"; entry: HistoryEntry }
  | { action: "updated"; entry: HistoryEntry }
  | { action: "deleted"; id: number }
  | { action: "cleared" }
  | { action: "toggled"; id: number; saved: boolean };

interface HistoryActionsProps {
  deleteAllDisabled: boolean;
  onDeleteAll: () => void;
  onOpenRecordings: () => void;
}

const HistoryActions: React.FC<HistoryActionsProps> = ({
  deleteAllDisabled,
  onDeleteAll,
  onOpenRecordings,
}) => {
  const { t } = useTranslation();

  return (
    <div className="flex flex-wrap items-center justify-end gap-2">
      <Button
        onClick={onDeleteAll}
        variant="danger"
        size="sm"
        className="flex items-center gap-2"
        title={t("settings.history.deleteAll")}
        disabled={deleteAllDisabled}
      >
        <Trash2 className="w-4 h-4" />
        <span>{t("settings.history.deleteAll")}</span>
      </Button>
      <Button
        onClick={onOpenRecordings}
        variant="secondary"
        size="sm"
        className="flex items-center gap-2"
        title={t("settings.history.openFolder")}
      >
        <FolderOpen className="w-4 h-4" />
        <span>{t("settings.history.openFolder")}</span>
      </Button>
    </div>
  );
};

const HistoryConfigurationSection: React.FC = () => {
  const { t } = useTranslation();

  return (
    <SettingsGroup title={t("settings.history.settings.title")}>
      <HistoryLimit descriptionMode="tooltip" grouped={true} />
      <RecordingRetentionPeriodSelector
        descriptionMode="tooltip"
        grouped={true}
      />
    </SettingsGroup>
  );
};

const normalizeCount = (value: unknown) => {
  const count = Number(value ?? 0);
  if (!Number.isFinite(count) || count < 0) {
    return 0;
  }
  return Math.trunc(count);
};

const formatCount = (value: number) => {
  return new Intl.NumberFormat().format(value);
};

interface DictationStatsRuntimeState {
  is_editing: boolean;
  is_recording: boolean;
  is_processing: boolean;
  can_start_edit: boolean;
  can_apply_edit: boolean;
  max_count: number;
  warning_threshold: number;
}

const DEFAULT_DICTATION_STATS_MAX = Number.MAX_SAFE_INTEGER;

const DEFAULT_DICTATION_STATS_RUNTIME_STATE: DictationStatsRuntimeState = {
  is_editing: false,
  is_recording: false,
  is_processing: false,
  can_start_edit: true,
  can_apply_edit: false,
  max_count: DEFAULT_DICTATION_STATS_MAX,
  warning_threshold:
    DEFAULT_DICTATION_STATS_MAX - Math.floor(DEFAULT_DICTATION_STATS_MAX / 20),
};

const normalizeCountDraft = (value: string) => {
  const digitsOnly = value.replace(/[^\d]/g, "");
  if (!digitsOnly) {
    return "";
  }
  return digitsOnly.replace(/^0+(?=\d)/, "");
};

const parseCountDraft = (value: string) => {
  if (!value) {
    return null;
  }

  try {
    return BigInt(value);
  } catch {
    return null;
  }
};

const clampCountDraft = (value: string, maxCount: number) => {
  const normalized = normalizeCountDraft(value);
  if (!normalized) {
    return { value: "", clamped: false };
  }

  const parsed = parseCountDraft(normalized);
  if (parsed === null) {
    return { value: normalized, clamped: false };
  }

  const maxCountBigInt = BigInt(maxCount);
  if (parsed > maxCountBigInt) {
    return { value: maxCountBigInt.toString(), clamped: true };
  }

  return { value: normalized, clamped: false };
};

const DictationStatsSection: React.FC = () => {
  const { t, i18n } = useTranslation();
  const { settings, updateSetting, isUpdating, refreshSettings } =
    useSettings();
  const [resetting, setResetting] = useState<
    "words" | "characters" | "both" | null
  >(null);
  const [runtimeState, setRuntimeState] = useState<DictationStatsRuntimeState>(
    DEFAULT_DICTATION_STATS_RUNTIME_STATE,
  );
  const [isEditing, setIsEditing] = useState(false);
  const [savingEdit, setSavingEdit] = useState(false);
  const [wordDraft, setWordDraft] = useState("");
  const [characterDraft, setCharacterDraft] = useState("");
  const [draftWasClamped, setDraftWasClamped] = useState(false);

  const syncRuntimeState = useCallback(async () => {
    try {
      const nextState = await invoke<DictationStatsRuntimeState>(
        "get_dictation_stats_runtime_state",
      );
      setRuntimeState(nextState);
    } catch (error) {
      console.error("Failed to load dictation stats runtime state:", error);
    }
  }, []);

  useEffect(() => {
    const unlisten = listen("dictation-stats-updated", () => {
      refreshSettings();
    });

    return () => {
      unlisten.then((cleanup) => cleanup());
    };
  }, [refreshSettings]);

  useEffect(() => {
    void syncRuntimeState();

    const unlisten = listen("dictation-stats-editing-changed", () => {
      void syncRuntimeState();
    });

    return () => {
      unlisten.then((cleanup) => cleanup());
    };
  }, [syncRuntimeState]);

  useEffect(() => {
    if (!isEditing) {
      return;
    }

    void syncRuntimeState();
    const timerId = window.setInterval(() => {
      void syncRuntimeState();
    }, 800);

    return () => {
      window.clearInterval(timerId);
    };
  }, [isEditing, syncRuntimeState]);

  useEffect(() => {
    return () => {
      if (isEditing) {
        void invoke("cancel_dictation_stats_edit_session");
      }
    };
  }, [isEditing]);

  const statsEnabled = Boolean((settings as any)?.dictation_stats_enabled);
  const wordCount = normalizeCount((settings as any)?.dictation_word_count);
  const characterCount = normalizeCount(
    (settings as any)?.dictation_character_count,
  );
  const wordCountSinceMs = Number(
    (settings as any)?.dictation_word_count_since_ms || 0,
  );
  const charCountSinceMs = Number(
    (settings as any)?.dictation_character_count_since_ms || 0,
  );

  const wordSince = wordCountSinceMs
    ? formatDate(String(Math.floor(wordCountSinceMs / 1000)), i18n.language)
    : null;
  const charSince = charCountSinceMs
    ? formatDate(String(Math.floor(charCountSinceMs / 1000)), i18n.language)
    : null;
  const sameDate = wordSince === charSince;
  const formattedMaxCount = formatCount(runtimeState.max_count);
  const maxCountBigInt = BigInt(runtimeState.max_count);
  const warningThresholdBigInt = BigInt(runtimeState.warning_threshold);
  const parsedWordDraft = parseCountDraft(wordDraft);
  const parsedCharacterDraft = parseCountDraft(characterDraft);
  const isBusyDuringEdit =
    runtimeState.is_recording || runtimeState.is_processing;
  const canStartEditing =
    resetting === null &&
    !savingEdit &&
    (runtimeState.can_start_edit || runtimeState.is_editing);
  const canSaveEdit =
    resetting === null &&
    !savingEdit &&
    runtimeState.can_apply_edit &&
    parsedWordDraft !== null &&
    parsedCharacterDraft !== null;
  const currentAtLimit =
    wordCount >= runtimeState.max_count ||
    characterCount >= runtimeState.max_count;
  const currentNearLimit =
    wordCount >= runtimeState.warning_threshold ||
    characterCount >= runtimeState.warning_threshold;
  const draftAtLimit =
    (parsedWordDraft !== null && parsedWordDraft >= maxCountBigInt) ||
    (parsedCharacterDraft !== null && parsedCharacterDraft >= maxCountBigInt);
  const draftNearLimit =
    (parsedWordDraft !== null && parsedWordDraft >= warningThresholdBigInt) ||
    (parsedCharacterDraft !== null &&
      parsedCharacterDraft >= warningThresholdBigInt);

  const resetStats = async (
    kind: "words" | "characters" | "both",
    command: string,
  ) => {
    setResetting(kind);
    try {
      await invoke(command);
      await refreshSettings();
    } catch (error) {
      console.error("Failed to reset dictation stats:", error);
      toast.error(t("settings.history.dictationStats.resetError"));
    } finally {
      setResetting(null);
    }
  };

  const startEditing = async () => {
    try {
      await invoke("begin_dictation_stats_edit_session");
      setWordDraft(String(wordCount));
      setCharacterDraft(String(characterCount));
      setDraftWasClamped(false);
      setIsEditing(true);
      await syncRuntimeState();
    } catch (error) {
      console.error("Failed to start dictation stats editing:", error);
      toast.error(
        t("settings.history.dictationStats.edit.startError", {
          defaultValue:
            "Failed to start dictation stats editing. Please try again.",
        }),
      );
    }
  };

  const cancelEditing = async () => {
    try {
      await invoke("cancel_dictation_stats_edit_session");
    } catch (error) {
      console.error("Failed to cancel dictation stats editing:", error);
    } finally {
      setIsEditing(false);
      setDraftWasClamped(false);
      await syncRuntimeState();
    }
  };

  const handleDraftChange = (
    value: string,
    setter: React.Dispatch<React.SetStateAction<string>>,
  ) => {
    const nextDraft = clampCountDraft(value, runtimeState.max_count);
    setter(nextDraft.value);
    if (nextDraft.clamped) {
      setDraftWasClamped(true);
    }
  };

  const saveEditedCounts = async () => {
    if (
      !canSaveEdit ||
      parsedWordDraft === null ||
      parsedCharacterDraft === null
    ) {
      toast.error(
        t("settings.history.dictationStats.edit.invalid", {
          defaultValue: "Enter a valid whole number for both counters.",
        }),
      );
      return;
    }

    setSavingEdit(true);
    try {
      await invoke("update_dictation_stats_counts", {
        wordCount: Number(parsedWordDraft),
        characterCount: Number(parsedCharacterDraft),
      });
      await refreshSettings();
      setIsEditing(false);
      setDraftWasClamped(false);
    } catch (error) {
      console.error("Failed to update dictation stats:", error);
      toast.error(
        t("settings.history.dictationStats.edit.saveError", {
          defaultValue: "Failed to update dictation stats. Please try again.",
        }),
      );
    } finally {
      setSavingEdit(false);
      await syncRuntimeState();
    }
  };

  return (
    <SettingsGroup title={t("settings.history.dictationStats.title")}>
      <ToggleSwitch
        checked={statsEnabled}
        onChange={async (enabled) => {
          await updateSetting("dictation_stats_enabled" as any, enabled as any);
          await refreshSettings();
        }}
        disabled={isEditing || savingEdit}
        isUpdating={isUpdating("dictation_stats_enabled")}
        label={t("settings.history.dictationStats.enable.title")}
        description={t("settings.history.dictationStats.enable.description")}
        descriptionMode="tooltip"
        grouped={true}
      />
      <SettingContainer
        title={t("settings.history.dictationStats.counts.title")}
        description={t("settings.history.dictationStats.counts.description")}
        descriptionMode="tooltip"
        grouped={true}
      >
        <div className="flex flex-col items-end gap-1">
          <div className="flex flex-wrap items-center justify-end gap-2 text-sm">
            <span className="rounded bg-mid-gray/10 px-2 py-1 text-text/90">
              {t("settings.history.dictationStats.words", {
                count: wordCount,
                formattedCount: formatCount(wordCount),
              })}
            </span>
            <span className="rounded bg-mid-gray/10 px-2 py-1 text-text/90">
              {t("settings.history.dictationStats.characters", {
                count: characterCount,
                formattedCount: formatCount(characterCount),
              })}
            </span>
          </div>
          {sameDate && wordSince && (
            <div className="text-xs text-text/50">
              {t("settings.history.dictationStats.since", { date: wordSince })}
            </div>
          )}
          {!sameDate && (wordSince || charSince) && (
            <div className="text-[10px] text-text/40">
              {wordSince &&
                t("settings.history.dictationStats.sinceWords", {
                  defaultValue: "Words since {{date}}",
                  date: wordSince,
                })}
              {wordSince && charSince && " • "}
              {charSince &&
                t("settings.history.dictationStats.sinceCharacters", {
                  defaultValue: "Characters since {{date}}",
                  date: charSince,
                })}
            </div>
          )}
        </div>
        {!isEditing && currentAtLimit && (
          <Alert variant="warning" contained={true} className="mt-3">
            {t("settings.history.dictationStats.edit.limitReached", {
              defaultValue:
                "This counter is at the safety limit. Further dictation will not increase it until you lower or reset it.",
              formattedMax: formattedMaxCount,
            })}
          </Alert>
        )}
        {!isEditing && !currentAtLimit && currentNearLimit && (
          <Alert variant="warning" contained={true} className="mt-3">
            {t("settings.history.dictationStats.edit.nearLimit", {
              defaultValue:
                "This counter is near the safety limit. Once it reaches {{formattedMax}}, further dictation stops increasing it until you lower or reset it.",
              formattedMax: formattedMaxCount,
            })}
          </Alert>
        )}
      </SettingContainer>
      <SettingContainer
        title={t("settings.history.dictationStats.reset.title")}
        description={t("settings.history.dictationStats.reset.description")}
        descriptionMode="tooltip"
        grouped={true}
      >
        {isEditing ? (
          <div className="flex flex-col gap-3">
            <Alert variant="info" contained={true}>
              {t("settings.history.dictationStats.edit.paused", {
                defaultValue:
                  "Counting pauses while you are editing these values.",
              })}
            </Alert>
            {isBusyDuringEdit && (
              <Alert variant="warning" contained={true}>
                {t("settings.history.dictationStats.edit.busy", {
                  defaultValue:
                    "Finish the current recording or processing task before saving changes.",
                })}
              </Alert>
            )}
            {draftWasClamped && (
              <Alert variant="warning" contained={true}>
                {t("settings.history.dictationStats.edit.clamped", {
                  defaultValue:
                    "Very large values are automatically capped at {{formattedMax}}.",
                  formattedMax: formattedMaxCount,
                })}
              </Alert>
            )}
            {draftAtLimit && (
              <Alert variant="warning" contained={true}>
                {t("settings.history.dictationStats.edit.limitReached", {
                  defaultValue:
                    "This counter is at the safety limit. Further dictation will not increase it until you lower or reset it.",
                  formattedMax: formattedMaxCount,
                })}
              </Alert>
            )}
            {!draftAtLimit && draftNearLimit && (
              <Alert variant="warning" contained={true}>
                {t("settings.history.dictationStats.edit.nearLimit", {
                  defaultValue:
                    "This counter is near the safety limit. Once it reaches {{formattedMax}}, further dictation stops increasing it until you lower or reset it.",
                  formattedMax: formattedMaxCount,
                })}
              </Alert>
            )}
            <div className="grid gap-3 sm:grid-cols-2">
              <label className="flex flex-col gap-2 text-xs font-medium uppercase tracking-[0.12em] text-text/50">
                <span>
                  {t("settings.history.dictationStats.edit.wordsLabel", {
                    defaultValue: "Words",
                  })}
                </span>
                <Input
                  value={wordDraft}
                  onChange={(event) =>
                    handleDraftChange(event.target.value, setWordDraft)
                  }
                  inputMode="numeric"
                  pattern="[0-9]*"
                  placeholder="0"
                  disabled={savingEdit}
                />
              </label>
              <label className="flex flex-col gap-2 text-xs font-medium uppercase tracking-[0.12em] text-text/50">
                <span>
                  {t("settings.history.dictationStats.edit.charactersLabel", {
                    defaultValue: "Characters",
                  })}
                </span>
                <Input
                  value={characterDraft}
                  onChange={(event) =>
                    handleDraftChange(event.target.value, setCharacterDraft)
                  }
                  inputMode="numeric"
                  pattern="[0-9]*"
                  placeholder="0"
                  disabled={savingEdit}
                />
              </label>
            </div>
            <div className="text-xs text-text/50">
              {t("settings.history.dictationStats.edit.hint", {
                defaultValue:
                  "Only digits are allowed. Values are capped at {{formattedMax}} to avoid overflow and precision issues.",
                formattedMax: formattedMaxCount,
              })}
            </div>
            <div className="flex flex-wrap items-center justify-end gap-2">
              <Button
                variant="ghost"
                size="sm"
                disabled={savingEdit}
                onClick={() => void cancelEditing()}
              >
                {t("settings.history.dictationStats.edit.cancel", {
                  defaultValue: "Cancel",
                })}
              </Button>
              <Button
                size="sm"
                disabled={!canSaveEdit}
                onClick={() => void saveEditedCounts()}
              >
                {savingEdit
                  ? t("settings.history.dictationStats.edit.saving", {
                      defaultValue: "Saving...",
                    })
                  : t("settings.history.dictationStats.edit.save", {
                      defaultValue: "Save",
                    })}
              </Button>
            </div>
          </div>
        ) : (
          <div className="flex flex-wrap items-center justify-end gap-2">
            <Button
              variant="secondary"
              size="sm"
              disabled={!canStartEditing}
              onClick={() => void startEditing()}
            >
              {t("settings.history.dictationStats.edit.button", {
                defaultValue: "Edit",
              })}
            </Button>
            <Button
              variant="secondary"
              size="sm"
              disabled={resetting !== null}
              onClick={() => resetStats("words", "reset_dictation_word_count")}
            >
              {resetting === "words"
                ? t("settings.history.dictationStats.resetting")
                : t("settings.history.dictationStats.reset.words")}
            </Button>
            <Button
              variant="secondary"
              size="sm"
              disabled={resetting !== null}
              onClick={() =>
                resetStats("characters", "reset_dictation_character_count")
              }
            >
              {resetting === "characters"
                ? t("settings.history.dictationStats.resetting")
                : t("settings.history.dictationStats.reset.characters")}
            </Button>
            <Button
              variant="secondary"
              size="sm"
              disabled={resetting !== null}
              onClick={() => resetStats("both", "reset_dictation_stats")}
            >
              {resetting === "both"
                ? t("settings.history.dictationStats.resetting")
                : t("settings.history.dictationStats.reset.both")}
            </Button>
          </div>
        )}
      </SettingContainer>
    </SettingsGroup>
  );
};

const IconButton: React.FC<{
  onClick: () => void;
  title: string;
  tooltip?: React.ReactNode;
  disabled?: boolean;
  active?: boolean;
  children: React.ReactNode;
}> = ({ onClick, title, tooltip, disabled, active, children }) => {
  const button = (
  <button
    type="button"
    onClick={onClick}
    disabled={disabled}
    aria-label={title}
    className={`p-1.5 rounded-md flex items-center justify-center transition-colors cursor-pointer disabled:cursor-not-allowed disabled:text-text/20 ${
      active
        ? "text-logo-primary hover:text-logo-primary/80"
        : "text-text/50 hover:text-logo-primary"
    } ${tooltip && disabled ? "pointer-events-none" : ""}`}
    title={tooltip ? undefined : title}
  >
    {children}
  </button>
  );
  return tooltip ? <Tooltip content={tooltip}>{button}</Tooltip> : button;
};

const RepasteShortcutSection: React.FC = () => {
  const { t } = useTranslation();

  return (
    <SettingsGroup title={t("settings.history.shortcut.title")}>
      <HandyShortcut
        shortcutId="repaste_last"
        grouped={true}
        descriptionMode="inline"
      />
    </SettingsGroup>
  );
};

const HistoryPageLayout: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <HistoryConfigurationSection />
      <DictationStatsSection />
      <RepasteShortcutSection />
      {children}
    </div>
  );
};

export const HistorySettings: React.FC = () => {
  const { t } = useTranslation();
  const [historyEntries, setHistoryEntries] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [hasMore, setHasMore] = useState(true);
  const [searchInput, setSearchInput] = useState("");
  const [searchQuery, setSearchQuery] = useState("");
  const [showDeleteAllConfirm, setShowDeleteAllConfirm] = useState(false);
  const [isDeletingAll, setIsDeletingAll] = useState(false);
  const sentinelRef = useRef<HTMLDivElement>(null);
  const entriesRef = useRef<HistoryEntry[]>([]);
  const loadingRef = useRef(false);
  const historyRequestRef = useRef(0);
  const searchQueryRef = useRef("");
  const listenerReadyRef = useRef(false);
  // Keep the server cursor even if live edits remove the last visible match.
  const pageCursorRef = useRef<number | null>(null);
  // Search may include a newly matching update, but never resurrect a deleted row.
  const deletedIdsRef = useRef(new Set<number>());
  const historyClearedRef = useRef(false);
  const addedIdsAfterClearRef = useRef(new Set<number>());
  const pendingHistoryChangesRef = useRef<HistoryUpdatePayload[]>([]);
  const pendingToggleIdsRef = useRef<Set<number>>(new Set());
  const [retryStates, setRetryStates] = useState<Record<number, HistoryRetryState>>({});
  const retriesInFlightRef = useRef(new Set<number>());
  const retriesMountedRef = useRef(true);
  const retryCancellationRef = useRef(new Set<number>());
  const cancelHistoryReplay = useCallback(async (id: number) => {
    if (!retriesInFlightRef.current.has(id)) return;
    retryCancellationRef.current.add(id);
    await invoke("cancel_history_entry_transcription", { id });
  }, []);

  useEffect(() => {
    retriesMountedRef.current = true;
    return () => {
      retriesMountedRef.current = false;
      for (const id of retriesInFlightRef.current) {
        void cancelHistoryReplay(id).catch(console.error);
      }
    };
  }, [cancelHistoryReplay]);

  useEffect(() => {
    entriesRef.current = historyEntries;
  }, [historyEntries]);

  const loadHistoryEntries = useCallback(async (cursor?: number) => {
    const isFirstPage = cursor === undefined;
    if (!isFirstPage && loadingRef.current) {
      return;
    }

    loadingRef.current = true;
    const request = ++historyRequestRef.current;
    const query = searchQueryRef.current;
    pendingHistoryChangesRef.current = [];
    setLoading(true);

    try {
      const result = await invoke<PaginatedHistory>("get_history_entries", {
        cursor: cursor ?? null,
        limit: PAGE_SIZE,
        search: query || null,
      });

      if (request !== historyRequestRef.current) return;
      const changes = pendingHistoryChangesRef.current.slice();
      const nextEntries = result.entries ?? [];
      setHistoryEntries((prev) =>
        reconcileHistoryPage(
          prev, nextEntries, isFirstPage, changes,
          query ? (entry) => matchesHistorySearch(entry, query) : undefined,
        ),
      );
      pageCursorRef.current = nextEntries[nextEntries.length - 1]?.id ?? null;
      setHasMore(!changes.some((change) => change.action === "cleared") && Boolean(result.has_more));
    } catch (error) {
      console.error("Failed to load history entries:", error);
    } finally {
      if (request === historyRequestRef.current) {
        setLoading(false);
        loadingRef.current = false;
        pendingHistoryChangesRef.current = [];
      }
    }
  }, []);

  const applySearch = useCallback(
    (query: string) => {
      if (query === searchQueryRef.current) return;
      searchQueryRef.current = query;
      setSearchQuery(query);
      if (listenerReadyRef.current) void loadHistoryEntries();
    },
    [loadHistoryEntries],
  );

  useEffect(() => {
    const timeout = window.setTimeout(
      () => applySearch(searchInput.trim()), SEARCH_DEBOUNCE_MS,
    );
    return () => window.clearTimeout(timeout);
  }, [searchInput, applySearch]);

  const clearSearch = () => {
    setSearchInput("");
    applySearch("");
  };

  useEffect(() => {
    if (loading) {
      return;
    }

    const sentinel = sentinelRef.current;
    if (!sentinel || !hasMore) {
      return;
    }

    const observer = new IntersectionObserver(
      (observerEntries) => {
        const first = observerEntries[0];
        if (!first.isIntersecting) {
          return;
        }

        const cursor = pageCursorRef.current;
        if (cursor !== null) {
          void loadHistoryEntries(cursor);
        }
      },
      { threshold: 0 },
    );

    observer.observe(sentinel);

    return () => {
      observer.disconnect();
    };
  }, [hasMore, loadHistoryEntries, loading]);

  useEffect(() => {
    let disposed = false;
    const setupListener = async () => {
      const unlisten = await listen<HistoryUpdatePayload>(
        "history-update-payload",
        (event) => {
          if (disposed) return;
          const payload = event.payload;
          if (payload.action === "deleted") {
            void cancelHistoryReplay(payload.id).catch(console.error);
          } else if (payload.action === "cleared") {
            for (const id of retriesInFlightRef.current) {
              void cancelHistoryReplay(id).catch(console.error);
            }
          }
          if (payload.action === "added") {
            deletedIdsRef.current.delete(payload.entry.id);
            if (historyClearedRef.current) addedIdsAfterClearRef.current.add(payload.entry.id);
          }
          if (payload.action === "deleted") deletedIdsRef.current.add(payload.id);
          if (payload.action === "cleared") {
            historyClearedRef.current = true;
            addedIdsAfterClearRef.current.clear();
          }
          if (payload.action === "updated" && deletedIdsRef.current.has(payload.entry.id)) return;
          if (loadingRef.current) pendingHistoryChangesRef.current.push(payload);
          const query = searchQueryRef.current;
          const mayIncludeUpdate =
            !historyClearedRef.current ||
            (payload.action === "updated" && addedIdsAfterClearRef.current.has(payload.entry.id));
          setHistoryEntries((prev) =>
            reconcileHistoryPage(
              prev, [], false, [payload],
              query && mayIncludeUpdate
                ? (entry) => matchesHistorySearch(entry, query)
                : undefined,
            ).filter((entry) => matchesHistorySearch(entry, query)),
          );
          if (payload.action === "cleared") {
            setHasMore(false);
          }
        },
      );

      // Subscribe before reading the snapshot so no intervening change is lost.
      if (!disposed) {
        listenerReadyRef.current = true;
        void loadHistoryEntries();
      }
      return unlisten;
    };

    const unlistenPromise = setupListener().catch((error) => {
      console.error("Failed to subscribe to history updates:", error);
      if (!disposed) {
        listenerReadyRef.current = true;
        void loadHistoryEntries();
      }
    });

    return () => {
      disposed = true;
      listenerReadyRef.current = false;
      ++historyRequestRef.current;
      unlistenPromise.then((unlisten) => {
        if (unlisten) {
          unlisten();
        }
      });
    };
  }, [loadHistoryEntries, cancelHistoryReplay]);

  const toggleSaved = async (id: number) => {
    if (pendingToggleIdsRef.current.has(id)) {
      return;
    }
    pendingToggleIdsRef.current.add(id);
    setHistoryEntries((prev) =>
      prev.map((entry) =>
        entry.id === id ? { ...entry, saved: !entry.saved } : entry,
      ),
    );

    try {
      const result = await commands.toggleHistoryEntrySaved(id);
      if (result.status === "error") {
        setHistoryEntries((prev) =>
          prev.map((entry) =>
            entry.id === id ? { ...entry, saved: !entry.saved } : entry,
          ),
        );
      }
    } catch (error) {
      console.error("Failed to toggle saved status:", error);
      setHistoryEntries((prev) =>
        prev.map((entry) =>
          entry.id === id ? { ...entry, saved: !entry.saved } : entry,
        ),
      );
    } finally {
      pendingToggleIdsRef.current.delete(id);
    }
  };

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch (error) {
      console.error("Failed to copy to clipboard:", error);
      return false;
    }
  };

  const getAudioUrl = useCallback(async (fileName: string) => {
    try {
      const result = await commands.getAudioFilePath(fileName);
      if (result.status === "ok") {
        // WebKitGTK cannot stream audio from the asset protocol, so Linux
        // plays the recording from an in-memory blob instead.
        if (IS_LINUX) {
          const fileData = await readFile(result.data);
          return URL.createObjectURL(new Blob([fileData], { type: "audio/wav" }));
        }
        return convertFileSrc(`${result.data}`, "asset");
      }
      return null;
    } catch (error) {
      console.error("Failed to get audio file path:", error);
      return null;
    }
  }, []);

  const deleteAudioEntry = async (id: number) => {
    void cancelHistoryReplay(id).catch(console.error);
    const previousEntries = entriesRef.current;
    const deletedIndex = previousEntries.findIndex((entry) => entry.id === id);
    const deletedEntry =
      deletedIndex >= 0 ? previousEntries[deletedIndex] : null;

    setHistoryEntries((prev) => prev.filter((entry) => entry.id !== id));

    const restoreDeletedEntry = () => {
      setHistoryEntries((prev) => {
        if (
          !deletedEntry ||
          !matchesHistorySearch(deletedEntry, searchQueryRef.current) ||
          prev.some((entry) => entry.id === id)
        ) {
          return prev;
        }

        const nextEntries = [...prev];
        nextEntries.splice(
          Math.min(deletedIndex, nextEntries.length),
          0,
          deletedEntry,
        );
        return nextEntries;
      });
    };

    try {
      const result = await commands.deleteHistoryEntry(id);
      if (result.status === "error") {
        throw toHistoryDeleteCommandError(result.error);
      }
    } catch (error) {
      console.error("Failed to delete audio entry:", error);
      const deletionError = toHistoryDeleteCommandError(error);
      if (deletionError.payload.remaining_count > 0) {
        restoreDeletedEntry();
      }
      throw deletionError;
    }
  };

  const deleteAllHistoryEntries = async () => {
    setIsDeletingAll(true);
    for (const id of retriesInFlightRef.current) {
      void cancelHistoryReplay(id).catch(console.error);
    }

    try {
      const result = await commands.deleteAllHistoryEntries();
      if (result.status === "error") {
        throw toHistoryDeleteCommandError(result.error);
      }
      await loadHistoryEntries();
      toast.success(t("settings.history.deleteAllSuccess"));
    } catch (error) {
      console.error("Failed to delete all history entries:", error);
      await loadHistoryEntries();
      showHistoryDeleteFailure(t, error, () => {
        void deleteAllHistoryEntries();
      });
    } finally {
      setIsDeletingAll(false);
    }
  };

  const retryHistoryEntry = async (id: number) => {
    if (!retriesMountedRef.current || retriesInFlightRef.current.has(id)) return;
    retriesInFlightRef.current.add(id);
    setRetryStates((prev) => ({ ...prev, [id]: { progress: null, stopping: false } }));
    let unlisten: (() => void) | undefined;
    try {
      // Page ownership survives filtering a row out of the search results.
      unlisten = await listen<HistoryReplayProgress>("history-replay-progress", ({ payload }) => {
        if (payload.id !== id || !retriesInFlightRef.current.has(id)) return;
        if (!retriesMountedRef.current || retryCancellationRef.current.has(id)) {
          // Page cleanup can run before the backend registers its replay.
          void invoke("cancel_history_entry_transcription", { id }).catch(console.error);
          return;
        }
        setRetryStates((prev) => ({
          ...prev, [id]: { progress: payload, stopping: prev[id]?.stopping ?? false },
        }));
      });
      if (!retriesMountedRef.current || retryCancellationRef.current.has(id)) return;
      await invoke("retry_history_entry_transcription", { id });
    } catch (error) {
      if (!retriesMountedRef.current) return;
      if (String(error) === "History replay cancelled") {
        toast.info(t("settings.history.liveReplay.cancelled"));
      } else {
        console.error("Failed to re-transcribe:", error);
        toast.error(t("settings.history.retranscribeError"));
      }
    } finally {
      unlisten?.();
      retriesInFlightRef.current.delete(id);
      retryCancellationRef.current.delete(id);
      if (retriesMountedRef.current) {
        setRetryStates((prev) => {
          const next = { ...prev };
          delete next[id];
          return next;
        });
      }
    }
  };

  const stopHistoryReplay = async (id: number) => {
    if (!retriesInFlightRef.current.has(id)) return;
    setRetryStates((prev) => ({
      ...prev, [id]: { progress: prev[id]?.progress ?? null, stopping: true },
    }));
    try {
      await cancelHistoryReplay(id);
    } catch (error) {
      if (!retriesMountedRef.current || !retriesInFlightRef.current.has(id)) return;
      console.error("Failed to stop history replay:", error);
      retryCancellationRef.current.delete(id);
      setRetryStates((prev) => ({
        ...prev, [id]: { progress: prev[id]?.progress ?? null, stopping: false },
      }));
      toast.error(t("settings.history.liveReplay.stopError"));
    }
  };

  const openRecordingsFolder = async () => {
    try {
      await commands.openRecordingsFolder();
    } catch (error) {
      console.error("Failed to open recordings folder:", error);
    }
  };

  const deleteAllModal = (
    <ConfirmationModal
      isOpen={showDeleteAllConfirm}
      onClose={() => setShowDeleteAllConfirm(false)}
      onConfirm={deleteAllHistoryEntries}
      title={t("settings.history.deleteAllConfirmTitle")}
      message={t("settings.history.deleteAllConfirmMessage")}
      confirmText={t("settings.history.deleteAllConfirmButton")}
      cancelText={t("common.cancel")}
      variant="danger"
    />
  );

  return (
    <>
      <HistoryPageLayout>
        {/* History Entries Section */}
        <div className="space-y-2">
          <div className="px-4 flex items-center justify-between gap-3">
            <div>
              <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
                {t("settings.history.title")}
              </h2>
            </div>
            <HistoryActions
              deleteAllDisabled={loading || isDeletingAll || historyEntries.length === 0}
              onDeleteAll={() => setShowDeleteAllConfirm(true)}
              onOpenRecordings={openRecordingsFolder}
            />
          </div>
          <div className="bg-background border border-mid-gray/20 rounded-lg overflow-visible">
            <div className="relative mx-4 my-3">
              <Search className="absolute start-3 top-1/2 -translate-y-1/2 w-4 h-4 text-text/40 pointer-events-none" />
              <input
                type="search"
                className="w-full ps-9 pe-9 py-2 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded-lg focus:outline-none focus:ring-1 focus:ring-logo-primary placeholder:text-text/40 [&::-webkit-search-cancel-button]:appearance-none"
                value={searchInput}
                onChange={(event) => setSearchInput(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Escape" && searchInput) {
                    event.preventDefault();
                    clearSearch();
                  }
                }}
                placeholder={t("settings.history.searchPlaceholder")}
                aria-label={t("settings.history.searchPlaceholder")}
              />
              {searchInput && (
                <button type="button" onClick={clearSearch}
                  className="absolute end-2 top-1/2 -translate-y-1/2 p-1 rounded-md text-text/50 hover:text-logo-primary cursor-pointer"
                  title={t("settings.history.clearSearch")} aria-label={t("settings.history.clearSearch")}>
                  <X className="w-4 h-4" />
                </button>
              )}
            </div>
            {loading && historyEntries.length === 0 ? (
              <div role="status" aria-live="polite" className="px-4 py-3 text-center text-text/60">
                {t("settings.history.loading")}
              </div>
            ) : historyEntries.length === 0 ? (
              <div role="status" aria-live="polite" className="flex flex-col items-center gap-3 px-4 py-6 text-center">
                {searchQuery ? (
                  <p className="text-sm text-text/60">{t("settings.history.noSearchResults", { query: searchQuery })}</p>
                ) : (
                  <>
                    <div>
                      <p className="text-sm font-medium text-text">{t("settings.history.emptyTitle")}</p>
                      <p className="mt-1 text-sm text-text/60">{t("settings.history.empty")}</p>
                    </div>
                    <Button variant="secondary" size="sm"
                      onClick={() => useNavigationStore.getState().setSection("general")}>
                      {t("settings.history.configureDictation")}
                    </Button>
                  </>
                )}
              </div>
            ) : (
            <AudioPlayerGroup>
              <div className="divide-y divide-mid-gray/20">
                {historyEntries.map((entry) => (
                  <HistoryEntryComponent
                    key={entry.id}
                    entry={entry}
                    searchQuery={searchQuery}
                    onToggleSaved={() => toggleSaved(entry.id)}
                    onCopyText={copyToClipboard}
                    getAudioUrl={getAudioUrl}
                    deleteAudio={deleteAudioEntry}
                    retryTranscription={retryHistoryEntry}
                    retryState={retryStates[entry.id]}
                    stopReplay={() => stopHistoryReplay(entry.id)}
                  />
                ))}
              </div>
            </AudioPlayerGroup>
            )}
            {hasMore && <div ref={sentinelRef} className="h-1" />}
          </div>
        </div>
      </HistoryPageLayout>
      {deleteAllModal}
    </>
  );
};

interface HistoryEntryProps {
  entry: HistoryEntry;
  searchQuery: string;
  onToggleSaved: () => void;
  onCopyText: (text: string) => Promise<boolean>;
  getAudioUrl: (fileName: string) => Promise<string | null>;
  deleteAudio: (id: number) => Promise<void>;
  retryTranscription: (id: number) => Promise<void>;
  retryState?: HistoryRetryState;
  stopReplay: () => Promise<void>;
}

const HistoryEntryComponent: React.FC<HistoryEntryProps> = ({
  entry,
  searchQuery,
  onToggleSaved,
  onCopyText,
  getAudioUrl,
  deleteAudio,
  retryTranscription,
  retryState,
  stopReplay,
}) => {
  const { t, i18n } = useTranslation();
  const benchmarkUpdating = useSettingsStore(
    (state) => state.isUpdating.post_process_benchmark_user_message,
  );
  const [showCopied, setShowCopied] = useState(false);
  const [copyBurst, setCopyBurst] = useState(0);
  const [showRaw, setShowRaw] = useState(false);
  const copyFeedbackTimeoutRef = useRef<number | null>(null);
  const retrying = retryState !== undefined;
  const replayProgress = retryState?.progress ?? null;
  const stoppingReplay = retryState?.stopping ?? false;
  const retryMountedRef = useRef(true);
  const [playableAudio, setPlayableAudio] = useState<{
    url: string;
    durationSeconds: number;
  } | null>(null);

  const isAiReplace = entry.action_type === "ai_replace";
  // Processed text is shown by default; the raw transcript is one click away.
  const rawToggle = rawTranscriptToggle(entry);
  const rawToggleLabel = t(rawToggle === "unchanged"
    ? "settings.history.processedTextUnchanged"
    : showRaw
      ? "settings.history.showProcessedText"
      : "settings.history.showRawTranscript");
  const displayText = rawToggle === "available" && showRaw
    ? entry.transcription_text
    : historyDisplayText(entry);
  const hasDisplayText = displayText.trim().length > 0;
  const hasAudioReference =
    !isAiReplace && entry.file_name.trim().length > 0;
  const hasPlayableAudio = playableAudio !== null;

  useEffect(() => {
    retryMountedRef.current = true;
    return () => {
      retryMountedRef.current = false;
    };
  }, [entry.id]);

  useEffect(() => {
    return () => {
      if (copyFeedbackTimeoutRef.current !== null) {
        window.clearTimeout(copyFeedbackTimeoutRef.current);
      }
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    let metadataAudio: HTMLAudioElement | null = null;
    let timeoutId: number | null = null;
    let blobUrlToRevoke: string | null = null;

    setPlayableAudio(null);
    if (!hasAudioReference) {
      return;
    }

    void getAudioUrl(entry.file_name).then((url) => {
      if (cancelled || !url) {
        if (url?.startsWith("blob:")) {
          URL.revokeObjectURL(url);
        }
        return;
      }
      if (url.startsWith("blob:")) {
        blobUrlToRevoke = url;
      }

      metadataAudio = document.createElement("audio");
      metadataAudio.preload = "metadata";

      const finish = (durationSeconds: number | null) => {
        if (timeoutId !== null) {
          window.clearTimeout(timeoutId);
          timeoutId = null;
        }
        if (!cancelled && durationSeconds !== null && durationSeconds >= 1) {
          setPlayableAudio({ url, durationSeconds });
        }
        metadataAudio?.removeAttribute("src");
        metadataAudio?.load();
        metadataAudio = null;
      };

      metadataAudio.onloadedmetadata = () => {
        const durationSeconds = metadataAudio?.duration ?? Number.NaN;
        finish(
          Number.isFinite(durationSeconds) && durationSeconds > 0
            ? durationSeconds
            : null,
        );
      };
      metadataAudio.onerror = () => finish(null);
      metadataAudio.src = url;
      metadataAudio.load();
      timeoutId = window.setTimeout(() => finish(null), 5000);
    });

    return () => {
      cancelled = true;
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
      }
      metadataAudio?.removeAttribute("src");
      metadataAudio?.load();
      if (blobUrlToRevoke) {
        URL.revokeObjectURL(blobUrlToRevoke);
      }
    };
  }, [entry.file_name, getAudioUrl, hasAudioReference]);

  const handleCopyText = async () => {
    if (!hasDisplayText || retrying) {
      return;
    }

    if (!(await onCopyText(displayText))) {
      setShowCopied(false);
      if (copyFeedbackTimeoutRef.current !== null) {
        window.clearTimeout(copyFeedbackTimeoutRef.current);
        copyFeedbackTimeoutRef.current = null;
      }
      toast.error(t("settings.history.copyError"));
      return;
    }

    if (copyFeedbackTimeoutRef.current !== null) {
      window.clearTimeout(copyFeedbackTimeoutRef.current);
    }
    setShowCopied(true);
    setCopyBurst((burst) => burst + 1);
    copyFeedbackTimeoutRef.current = window.setTimeout(() => {
      setShowCopied(false);
      copyFeedbackTimeoutRef.current = null;
    }, 2000);
  };

  const handleOpenLlmBenchmark = async () => {
    const store = useSettingsStore.getState();
    if (
      retrying || !entry.transcription_text.trim() ||
      store.isUpdating.post_process_benchmark_user_message
    ) return;

    try {
      await store.updateSetting(
        "post_process_benchmark_user_message",
        entry.transcription_text,
        { throwOnError: true },
      );
      if (!retryMountedRef.current) return;
      navigateToSettingsAnchor({
        activateSection: () => useNavigationStore.getState().setSection("postprocessing"),
        targetId: "settings-llm-benchmark",
      });
    } catch (error) {
      console.error("Failed to open history text in LLM benchmark:", error);
      toast.error(t("settings.history.openLlmBenchmarkError"));
    }
  };

  const handleDeleteEntry = async () => {
    try {
      await deleteAudio(entry.id);
    } catch (error) {
      console.error("Failed to delete entry:", error);
      showHistoryDeleteFailure(t, error, () => {
        void handleDeleteEntry();
      });
    }
  };

  const handleRetranscribe = async () => {
    if (retryMountedRef.current && !retrying) await retryTranscription(entry.id);
  };

  const formattedDate = formatDateTime(String(entry.timestamp), i18n.language);

  // Truncate text for display
  const truncateText = (text: string, maxLength: number) => {
    if (text.length <= maxLength) return text;
    return text.substring(0, maxLength) + "...";
  };

  return (
    <div className="px-4 py-2 pb-5 flex flex-col gap-3">
      <div className="flex flex-wrap justify-between items-center gap-2">
        <div className="flex items-center gap-2">
          {isAiReplace && (
            <Wand2 width={14} height={14} className="text-logo-primary" />
          )}
          <p className="text-sm font-medium">{formattedDate}</p>
          {isAiReplace && (
            <span className="text-xs bg-logo-primary/20 text-logo-primary px-2 py-0.5 rounded">
              {t("settings.history.aiReplace.badge")}
            </span>
          )}
        </div>
        <div className="flex flex-wrap items-center gap-1">
          {!isAiReplace && (
            <IconButton
              onClick={handleOpenLlmBenchmark}
              disabled={retrying || benchmarkUpdating || !entry.transcription_text.trim()}
              title={t("settings.history.openLlmBenchmark")}
              tooltip={<>
                <span className="font-medium">{t("settings.history.openLlmBenchmark")}</span>
                <br />
                {t("settings.history.openLlmBenchmarkDescription")}
              </>}
            >
              <svg width={16} height={16} viewBox="0 0 24 24" fill="none"
                stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <path d="M7 3h6M8 3v7l-4 7.5A2 2 0 0 0 5.8 20h8.4a2 2 0 0 0 1.8-2.5L12 10V3M6 15h8" />
                <path d="M17 3h4v4m0-4-4 4" />
              </svg>
            </IconButton>
          )}
          {rawToggle !== "none" && (
            <IconButton
              onClick={() => setShowRaw((raw) => !raw)}
              disabled={rawToggle === "unchanged"}
              active={rawToggle === "available" && showRaw}
              title={rawToggleLabel}
              tooltip={rawToggle === "unchanged" ? rawToggleLabel : undefined}
            >
              <FileText width={16} height={16} />
            </IconButton>
          )}
          <IconButton
            onClick={handleCopyText}
            disabled={!hasDisplayText || retrying}
            title={t("settings.history.copyToClipboard")}
          >
            <CopyFeedbackIcon copied={showCopied} burstKey={copyBurst} />
          </IconButton>
          <IconButton
            onClick={onToggleSaved}
            disabled={retrying}
            active={entry.saved}
            title={
              entry.saved
                ? t("settings.history.unsave")
                : t("settings.history.save")
            }
          >
            <Star
              width={16}
              height={16}
              fill={entry.saved ? "currentColor" : "none"}
            />
          </IconButton>
          {!isAiReplace && hasPlayableAudio && (
            <IconButton
              onClick={handleRetranscribe}
              disabled={retrying}
              title={t("settings.history.retranscribe")}
            >
              <RotateCcw
                width={16}
                height={16}
                style={
                  retrying
                    ? { animation: "spin 1s linear infinite reverse" }
                    : undefined
                }
              />
            </IconButton>
          )}
          <IconButton
            onClick={handleDeleteEntry}
            disabled={retrying}
            title={t("settings.history.delete")}
          >
            <Trash2 width={16} height={16} />
          </IconButton>
        </div>
      </div>

      {retrying && replayProgress && (
        <div className="rounded-md border border-mid-gray/30 px-3 py-2 flex flex-col gap-2">
          <div className="flex items-center justify-between gap-3">
            <div className="text-xs flex flex-col gap-1">
              <span>{t("settings.history.liveReplay.progress", {
                sent: replayProgress.sent_seconds.toLocaleString(i18n.language, { minimumFractionDigits: 1, maximumFractionDigits: 1 }),
                total: replayProgress.duration_seconds.toLocaleString(i18n.language, { minimumFractionDigits: 1, maximumFractionDigits: 1 }),
              })}</span>
              <span className="text-text/60">
                {t(`settings.history.liveReplay.${replayProgress.phase}`)}
              </span>
            </div>
            <Button variant="secondary" size="sm" onClick={stopReplay} disabled={stoppingReplay}>
              {t(stoppingReplay ? "settings.history.liveReplay.stopping" : "settings.history.liveReplay.stop")}
            </Button>
          </div>
          <progress
            className="w-full h-1 accent-logo-primary"
            aria-label={t("settings.history.liveReplay.label")}
            value={replayProgress.sent_seconds}
            max={replayProgress.duration_seconds}
          />
        </div>
      )}

      {isAiReplace ? (
        // AI Replace Entry Display
        <div className="space-y-2">
          {/* Instruction (transcription_text) */}
          {entry.transcription_text && (
            <div>
              <p className="text-xs text-mid-gray uppercase">
                {t("settings.history.aiReplace.instruction")}
              </p>
              <p className="italic text-text/90 text-sm select-text cursor-text">
                <HighlightedText text={entry.transcription_text} query={searchQuery} />
              </p>
            </div>
          )}

          {/* Original Selection (if any) */}
          {entry.original_selection && (
            <div>
              <p className="text-xs text-mid-gray uppercase">
                {t("settings.history.aiReplace.originalSelection")}
              </p>
              <p className="text-text/70 text-sm select-text cursor-text">
                <HighlightedText text={searchQuery ? entry.original_selection : truncateText(entry.original_selection, 150)} query={searchQuery} />
              </p>
            </div>
          )}

          {/* AI Response or Error */}
          <div>
            <p className="text-xs text-mid-gray uppercase">
              {t("settings.history.aiReplace.response")}
            </p>
            {entry.ai_response ? (
              <p className="text-text/90 text-sm select-text cursor-text">
                <HighlightedText text={entry.ai_response} query={searchQuery} />
              </p>
            ) : (
              <div className="flex items-center gap-2 text-amber-500">
                <AlertTriangle width={14} height={14} />
                <p className="text-sm">
                  {t("settings.history.aiReplace.noResponse")}
                </p>
              </div>
            )}
          </div>
        </div>
      ) : (
        // Regular Transcription Entry Display
        <>
          <p
            className={`italic text-sm pb-2 ${
              retrying
                ? ""
                : hasDisplayText
                  ? "text-text/90 select-text cursor-text whitespace-pre-wrap break-words"
                  : "text-text/40"
            }`}
            style={
              retrying
                ? { animation: "transcribe-pulse 3s ease-in-out infinite" }
                : undefined
            }
          >
            {retrying && (
              <style>{`
                @keyframes transcribe-pulse {
                  0%, 100% { color: color-mix(in srgb, var(--color-text) 40%, transparent); }
                  50% { color: color-mix(in srgb, var(--color-text) 90%, transparent); }
                }
              `}</style>
            )}
            {retrying
              ? t("settings.history.transcribing")
              : hasDisplayText
                ? <HighlightedText text={displayText} query={searchQuery} />
                : t("settings.history.transcriptionFailed")}
          </p>
          {playableAudio && (
            <AudioPlayer
              src={playableAudio.url}
              initialDurationSeconds={playableAudio.durationSeconds}
              className="w-full"
            />
          )}
        </>
      )}
    </div>
  );
};
