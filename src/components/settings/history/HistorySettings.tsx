import React, { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { AudioPlayer } from "../../ui/AudioPlayer";
import { Button } from "../../ui/Button";
import { ConfirmationModal } from "../../ui/ConfirmationModal";
import {
  Copy,
  Star,
  Check,
  Trash2,
  FolderOpen,
  Wand2,
  AlertTriangle,
  RotateCcw,
} from "lucide-react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { commands, type HistoryEntry } from "@/bindings";
import { formatDateTime, formatDate } from "@/utils/dateFormat";
import { HandyShortcut } from "../HandyShortcut";
import { HistoryLimit } from "../HistoryLimit";
import { RecordingRetentionPeriodSelector } from "../RecordingRetentionPeriod";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { SettingContainer } from "../../ui/SettingContainer";
import { useSettings } from "@/hooks/useSettings";

const PAGE_SIZE = 30;

interface PaginatedHistory {
  entries: HistoryEntry[];
  has_more: boolean;
}

type HistoryUpdatePayload =
  | { action: "added"; entry: HistoryEntry }
  | { action: "updated"; entry: HistoryEntry }
  | { action: "deleted"; id: number }
  | { action: "cleared" }
  | { action: "toggled"; id: number };

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

const DictationStatsSection: React.FC = () => {
  const { t, i18n } = useTranslation();
  const { settings, updateSetting, isUpdating, refreshSettings } =
    useSettings();
  const [resetting, setResetting] = useState<
    "words" | "characters" | "both" | null
  >(null);

  useEffect(() => {
    const unlisten = listen("dictation-stats-updated", () => {
      refreshSettings();
    });

    return () => {
      unlisten.then((cleanup) => cleanup());
    };
  }, [refreshSettings]);

  const statsEnabled = Boolean((settings as any)?.dictation_stats_enabled);
  const wordCount = normalizeCount((settings as any)?.dictation_word_count);
  const characterCount = normalizeCount(
    (settings as any)?.dictation_character_count,
  );
  const wordCountSinceMs = Number((settings as any)?.dictation_word_count_since_ms || 0);
  const charCountSinceMs = Number((settings as any)?.dictation_character_count_since_ms || 0);

  const wordSince = wordCountSinceMs ? formatDate(String(Math.floor(wordCountSinceMs / 1000)), i18n.language) : null;
  const charSince = charCountSinceMs ? formatDate(String(Math.floor(charCountSinceMs / 1000)), i18n.language) : null;
  const sameDate = wordSince === charSince;

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

  return (
    <SettingsGroup title={t("settings.history.dictationStats.title")}>
      <ToggleSwitch
        checked={statsEnabled}
        onChange={async (enabled) => {
          await updateSetting("dictation_stats_enabled" as any, enabled as any);
          await refreshSettings();
        }}
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
              {wordSince && `Words since ${wordSince}`}
              {wordSince && charSince && " • "}
              {charSince && `Characters since ${charSince}`}
            </div>
          )}
        </div>
      </SettingContainer>
      <SettingContainer
        title={t("settings.history.dictationStats.reset.title")}
        description={t("settings.history.dictationStats.reset.description")}
        descriptionMode="tooltip"
        grouped={true}
      >
        <div className="flex flex-wrap items-center justify-end gap-2">
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
      </SettingContainer>
    </SettingsGroup>
  );
};

const IconButton: React.FC<{
  onClick: () => void;
  title: string;
  disabled?: boolean;
  active?: boolean;
  children: React.ReactNode;
}> = ({ onClick, title, disabled, active, children }) => (
  <button
    onClick={onClick}
    disabled={disabled}
    className={`p-1.5 rounded-md flex items-center justify-center transition-colors cursor-pointer disabled:cursor-not-allowed disabled:text-text/20 ${
      active
        ? "text-logo-primary hover:text-logo-primary/80"
        : "text-text/50 hover:text-logo-primary"
    }`}
    title={title}
  >
    {children}
  </button>
);

const RepasteShortcutSection: React.FC = () => {
  const { t } = useTranslation();

  return (
    <SettingsGroup title={t("settings.history.shortcut.title")}>
      <HandyShortcut shortcutId="repaste_last" grouped={true} />
    </SettingsGroup>
  );
};

const HistoryPageLayout: React.FC<{ children: React.ReactNode }> = ({ children }) => {
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
  const [showDeleteAllConfirm, setShowDeleteAllConfirm] = useState(false);
  const [isDeletingAll, setIsDeletingAll] = useState(false);
  const sentinelRef = useRef<HTMLDivElement>(null);
  const entriesRef = useRef<HistoryEntry[]>([]);
  const loadingRef = useRef(false);
  const pendingToggleIdsRef = useRef<Set<number>>(new Set());

  useEffect(() => {
    entriesRef.current = historyEntries;
  }, [historyEntries]);

  const loadHistoryEntries = useCallback(async (cursor?: number) => {
    const isFirstPage = cursor === undefined;
    if (!isFirstPage && loadingRef.current) {
      return;
    }

    loadingRef.current = true;
    if (isFirstPage) {
      setLoading(true);
    }

    try {
      const result = await invoke<PaginatedHistory>("get_history_entries", {
        cursor: cursor ?? null,
        limit: PAGE_SIZE,
      });

      const nextEntries = result.entries ?? [];
      setHistoryEntries((prev) =>
        isFirstPage ? nextEntries : [...prev, ...nextEntries],
      );
      setHasMore(Boolean(result.has_more));
    } catch (error) {
      console.error("Failed to load history entries:", error);
    } finally {
      setLoading(false);
      loadingRef.current = false;
    }
  }, []);

  useEffect(() => {
    loadHistoryEntries();
  }, [loadHistoryEntries]);

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

        const lastEntry = entriesRef.current[entriesRef.current.length - 1];
        if (lastEntry) {
          loadHistoryEntries(lastEntry.id);
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
    const setupListener = async () => {
      const unlisten = await listen<HistoryUpdatePayload>(
        "history-update-payload",
        (event) => {
          const payload = event.payload;
          if (payload.action === "added") {
            setHistoryEntries((prev) => [payload.entry, ...prev]);
          } else if (payload.action === "updated") {
            setHistoryEntries((prev) =>
              prev.map((entry) =>
                entry.id === payload.entry.id ? payload.entry : entry,
              ),
            );
          } else if (payload.action === "deleted") {
            setHistoryEntries((prev) =>
              prev.filter((entry) => entry.id !== payload.id),
            );
          } else if (payload.action === "cleared") {
            setHistoryEntries([]);
            setHasMore(false);
          } else if (payload.action === "toggled") {
            if (pendingToggleIdsRef.current.has(payload.id)) {
              return;
            }
            setHistoryEntries((prev) =>
              prev.map((entry) =>
                entry.id === payload.id
                  ? { ...entry, saved: !entry.saved }
                  : entry,
              ),
            );
          }
        },
      );

      return unlisten;
    };

    const unlistenPromise = setupListener();

    return () => {
      unlistenPromise.then((unlisten) => {
        if (unlisten) {
          unlisten();
        }
      });
    };
  }, []);

  const toggleSaved = async (id: number) => {
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
    } catch (error) {
      console.error("Failed to copy to clipboard:", error);
    }
  };

  const getAudioUrl = useCallback(async (fileName: string) => {
    try {
      const result = await commands.getAudioFilePath(fileName);
      if (result.status === "ok") {
        return convertFileSrc(`${result.data}`, "asset");
      }
      return null;
    } catch (error) {
      console.error("Failed to get audio file path:", error);
      return null;
    }
  }, []);

  const deleteAudioEntry = async (id: number) => {
    const previousEntries = entriesRef.current;
    const deletedIndex = previousEntries.findIndex((entry) => entry.id === id);
    const deletedEntry =
      deletedIndex >= 0 ? previousEntries[deletedIndex] : null;

    setHistoryEntries((prev) => prev.filter((entry) => entry.id !== id));

    try {
      const result = await commands.deleteHistoryEntry(id);
      if (result.status === "error") {
        setHistoryEntries((prev) => {
          if (!deletedEntry || prev.some((entry) => entry.id === id)) {
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
      }
    } catch (error) {
      console.error("Failed to delete audio entry:", error);
      setHistoryEntries((prev) => {
        if (!deletedEntry || prev.some((entry) => entry.id === id)) {
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
      throw error;
    }
  };

  const deleteAllHistoryEntries = async () => {
    setIsDeletingAll(true);

    try {
      const result = await commands.deleteAllHistoryEntries();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      setHistoryEntries([]);
      setHasMore(false);
      toast.success(t("settings.history.deleteAllSuccess"));
    } catch (error) {
      console.error("Failed to delete all history entries:", error);
      toast.error(t("settings.history.deleteAllError"));
    } finally {
      setIsDeletingAll(false);
    }
  };

  const retryHistoryEntry = async (id: number) => {
    await invoke("retry_history_entry_transcription", { id });
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

  if (loading) {
    return (
      <>
        <HistoryPageLayout>
          <div className="space-y-2">
            <div className="px-4 flex items-center justify-between gap-3">
              <div>
                <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
                  {t("settings.history.title")}
                </h2>
              </div>
              <HistoryActions
                deleteAllDisabled={true}
                onDeleteAll={() => setShowDeleteAllConfirm(true)}
                onOpenRecordings={openRecordingsFolder}
              />
            </div>
            <div className="bg-background border border-mid-gray/20 rounded-lg overflow-visible">
              <div className="px-4 py-3 text-center text-text/60">
                {t("settings.history.loading")}
              </div>
            </div>
          </div>
        </HistoryPageLayout>
        {deleteAllModal}
      </>
    );
  }

  if (historyEntries.length === 0) {
    return (
      <>
        <HistoryPageLayout>
          <div className="space-y-2">
            <div className="px-4 flex items-center justify-between gap-3">
              <div>
                <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
                  {t("settings.history.title")}
                </h2>
              </div>
              <HistoryActions
                deleteAllDisabled={true}
                onDeleteAll={() => setShowDeleteAllConfirm(true)}
                onOpenRecordings={openRecordingsFolder}
              />
            </div>
            <div className="bg-background border border-mid-gray/20 rounded-lg overflow-visible">
              <div className="px-4 py-3 text-center text-text/60">
                {t("settings.history.empty")}
              </div>
            </div>
          </div>
        </HistoryPageLayout>
        {deleteAllModal}
      </>
    );
  }

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
              deleteAllDisabled={isDeletingAll || historyEntries.length === 0}
              onDeleteAll={() => setShowDeleteAllConfirm(true)}
              onOpenRecordings={openRecordingsFolder}
            />
          </div>
          <div className="bg-background border border-mid-gray/20 rounded-lg overflow-visible">
            <div className="divide-y divide-mid-gray/20">
              {historyEntries.map((entry) => (
                <HistoryEntryComponent
                  key={entry.id}
                  entry={entry}
                  onToggleSaved={() => toggleSaved(entry.id)}
                  onCopyText={() => {
                    const textToCopy =
                      entry.action_type === "ai_replace"
                        ? (entry.ai_response ?? entry.transcription_text)
                        : (entry.post_processed_text ??
                          entry.transcription_text);
                    copyToClipboard(textToCopy);
                  }}
                  getAudioUrl={getAudioUrl}
                  deleteAudio={deleteAudioEntry}
                  retryTranscription={retryHistoryEntry}
                />
              ))}
            </div>
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
  onToggleSaved: () => void;
  onCopyText: () => void;
  getAudioUrl: (fileName: string) => Promise<string | null>;
  deleteAudio: (id: number) => Promise<void>;
  retryTranscription: (id: number) => Promise<void>;
}

const HistoryEntryComponent: React.FC<HistoryEntryProps> = ({
  entry,
  onToggleSaved,
  onCopyText,
  getAudioUrl,
  deleteAudio,
  retryTranscription,
}) => {
  const { t, i18n } = useTranslation();
  const [showCopied, setShowCopied] = useState(false);
  const [retrying, setRetrying] = useState(false);

  const isAiReplace = entry.action_type === "ai_replace";
  const displayText = isAiReplace
    ? (entry.ai_response ?? entry.transcription_text)
    : (entry.post_processed_text ?? entry.transcription_text);
  const hasDisplayText = displayText.trim().length > 0;

  const handleLoadAudio = useCallback(
    () => getAudioUrl(entry.file_name),
    [getAudioUrl, entry.file_name],
  );

  const handleCopyText = () => {
    if (!hasDisplayText || retrying) {
      return;
    }

    onCopyText();
    setShowCopied(true);
    setTimeout(() => setShowCopied(false), 2000);
  };

  const handleDeleteEntry = async () => {
    try {
      await deleteAudio(entry.id);
    } catch (error) {
      console.error("Failed to delete entry:", error);
      toast.error(t("settings.history.deleteError"));
    }
  };

  const handleRetranscribe = async () => {
    try {
      setRetrying(true);
      await retryTranscription(entry.id);
    } catch (error) {
      console.error("Failed to re-transcribe:", error);
      toast.error(t("settings.history.retranscribeError"));
    } finally {
      setRetrying(false);
    }
  };

  const formattedDate = formatDateTime(String(entry.timestamp), i18n.language);

  // Truncate text for display
  const truncateText = (text: string, maxLength: number) => {
    if (text.length <= maxLength) return text;
    return text.substring(0, maxLength) + "...";
  };

  return (
    <div className="px-4 py-2 pb-5 flex flex-col gap-3">
      <div className="flex justify-between items-center">
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
        <div className="flex items-center gap-1">
          <IconButton
            onClick={handleCopyText}
            disabled={!hasDisplayText || retrying}
            title={t("settings.history.copyToClipboard")}
          >
            {showCopied ? (
              <Check width={16} height={16} />
            ) : (
              <Copy width={16} height={16} />
            )}
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
          {!isAiReplace && (
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
                {entry.transcription_text ||
                  t("settings.history.aiReplace.quickTap")}
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
                {truncateText(entry.original_selection, 150)}
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
                {entry.ai_response}
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
                ? displayText
                : t("settings.history.transcriptionFailed")}
          </p>
          <AudioPlayer onLoadRequest={handleLoadAudio} className="w-full" />
        </>
      )}
    </div>
  );
};
