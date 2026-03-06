import React, { useState, useRef, useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import {
  FileAudio,
  Upload,
  Copy,
  Check,
  Trash2,
  FileText,
  Loader2,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { stat } from "@tauri-apps/plugin-fs";
import {
  commands,
  DeepgramFileTranscriptionOptions,
  ModelInfo,
  SonioxFileTranscriptionOptions,
} from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { Button } from "@/components/ui/Button";
import { AudioPlayer } from "@/components/ui/AudioPlayer";
import { Dropdown } from "@/components/ui/Dropdown";
import { useTranscribeFileStore } from "@/stores/transcribeFileStore";
import { parseAndNormalizeSonioxLanguageHints } from "@/lib/constants/sonioxLanguages";

const supportedExtensions = ["wav", "mp3", "m4a", "ogg", "flac", "webm"];

type SpeakerNameSetProfile = {
  id: string;
  name: string;
  speaker_names: string[];
};

export const TranscribeFileSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();

  const {
    selectedFile,
    outputMode,
    outputFormat,
    overrideModelId,
    customWordsEnabledOverride,
    transcriptionResult,
    savedFilePath,
    isTranscribing,
    error,
    selectedProfileId,
    speakerArtifactPath,
    speakerProvider,
    speakerCards,
    isReapplyingSpeakerNames,
    setSelectedFile,
    setOutputMode,
    setOutputFormat,
    setOverrideModelId,
    setCustomWordsEnabledOverride,
    setTranscriptionResult,
    setSavedFilePath,
    setIsTranscribing,
    setError,
    setSelectedProfileId,
    setSpeakerSession,
    clearSpeakerSession,
    updateSpeakerCardName,
    applySpeakerCardNames,
    setIsReapplyingSpeakerNames,
  } = useTranscribeFileStore();
  const [isRecording, setIsRecording] = useState(false);
  const [copied, setCopied] = useState(false);
  const [isDragOver, setIsDragOver] = useState(false);
  const [availableModels, setAvailableModels] = useState<ModelInfo[]>([]);
  const [infoMessage, setInfoMessage] = useState<string | null>(null);
  const [sonioxLanguageHintsInput, setSonioxLanguageHintsInput] = useState("");
  const [sonioxEnableSpeakerDiarization, setSonioxEnableSpeakerDiarization] =
    useState(true);
  const [
    sonioxEnableLanguageIdentification,
    setSonioxEnableLanguageIdentification,
  ] = useState(true);
  const [deepgramFileDiarize, setDeepgramFileDiarize] = useState(false);
  const [deepgramFileMultichannel, setDeepgramFileMultichannel] =
    useState(false);
  const [selectedSpeakerNameProfileId, setSelectedSpeakerNameProfileId] =
    useState<string | null>(null);
  const [speakerNameProfileDraftName, setSpeakerNameProfileDraftName] =
    useState("");
  const [isSavingSpeakerNameProfile, setIsSavingSpeakerNameProfile] =
    useState(false);

  const dropZoneRef = useRef<HTMLDivElement>(null);
  const sonioxModel = (settings as any)?.soniox_model ?? "stt-rt-v4";
  const transcriptionProvider = String(settings?.transcription_provider ?? "local");
  const isSonioxProvider = transcriptionProvider === "remote_soniox";
  const isDeepgramProvider = transcriptionProvider === "remote_deepgram";
  const showSonioxFileOptions = !!selectedFile && isSonioxProvider && !overrideModelId;
  const showDeepgramFileOptions =
    !!selectedFile && isDeepgramProvider && !overrideModelId;
  const canReapplySpeakerNames =
    !!transcriptionResult &&
    !!speakerArtifactPath &&
    speakerCards.length > 0 &&
    !savedFilePath;
  const speakerProviderLabel =
    speakerProvider === "deepgram"
      ? "Deepgram"
      : speakerProvider === "soniox"
        ? "Soniox"
        : null;
  const settingsSonioxLanguageHints = (settings as any)?.soniox_language_hints as
    | string[]
    | undefined;
  const globalSonioxLanguageHints = useMemo(
    () => settingsSonioxLanguageHints ?? ["en"],
    [settingsSonioxLanguageHints],
  );
  const globalSonioxEnableLanguageIdentification = Boolean(
    (settings as any)?.soniox_enable_language_identification ?? true,
  );
  const globalSonioxEnableSpeakerDiarization = Boolean(
    (settings as any)?.soniox_enable_speaker_diarization ?? true,
  );
  const globalDeepgramFileDiarize = Boolean(
    (settings as any)?.deepgram_diarize ?? false,
  );
  const globalDeepgramFileMultichannel = Boolean(
    (settings as any)?.deepgram_multichannel ?? false,
  );

  useEffect(() => {
    setSonioxLanguageHintsInput(globalSonioxLanguageHints.join(", "));
    setSonioxEnableSpeakerDiarization(globalSonioxEnableSpeakerDiarization);
    setSonioxEnableLanguageIdentification(globalSonioxEnableLanguageIdentification);
  }, [
    globalSonioxEnableLanguageIdentification,
    globalSonioxEnableSpeakerDiarization,
    globalSonioxLanguageHints,
  ]);

  useEffect(() => {
    setDeepgramFileDiarize(globalDeepgramFileDiarize);
    setDeepgramFileMultichannel(globalDeepgramFileMultichannel);
  }, [globalDeepgramFileDiarize, globalDeepgramFileMultichannel]);

  // Listen for Tauri file drop events
  useEffect(() => {
    const appWindow = getCurrentWebviewWindow();
    
    const unlistenDrop = appWindow.onDragDropEvent(async (event) => {
      if (event.payload.type === "over") {
        setIsDragOver(true);
      } else if (event.payload.type === "leave") {
        setIsDragOver(false);
      } else if (event.payload.type === "drop") {
        setIsDragOver(false);
        const paths = event.payload.paths;
        if (paths && paths.length > 0) {
          const filePath = paths[0];
          const extension = filePath.split(".").pop()?.toLowerCase() ?? "";
          
          if (!supportedExtensions.includes(extension)) {
            setError(
              t("transcribeFile.unsupportedFormat", {
                format: extension,
                supported: supportedExtensions.join(", "),
              })
            );
            return;
          }
          
          const name = filePath.split(/[/\\]/).pop() ?? "unknown";
          
          // Get file size
          let fileSize = 0;
          try {
            const fileInfo = await stat(filePath);
            fileSize = fileInfo.size;
          } catch (e) {
            console.error("Failed to get file size:", e);
          }
          
          setSelectedFile({
            path: filePath,
            name,
            size: fileSize,
            audioUrl: convertFileSrc(filePath),
          });
          setTranscriptionResult("");
          setSavedFilePath(null);
          setInfoMessage(null);
          setError(null);
          clearSpeakerSession();
        }
      }
    });

    return () => {
      unlistenDrop.then((fn) => fn());
    };
  }, [t]);

  // Check recording state periodically
  useEffect(() => {
    const checkRecording = async () => {
      try {
        const isRec = await commands.isRecording();
        setIsRecording(isRec);
      } catch (e) {
        // Ignore errors
      }
    };

    checkRecording();
    const interval = setInterval(checkRecording, 500);
    return () => clearInterval(interval);
  }, []);

  // Fetch available models on mount
  useEffect(() => {
    commands.getAvailableModels().then((result) => {
        if (result.status === "ok") {
            // Filter only downloaded models
            setAvailableModels(result.data.filter(m => m.is_downloaded));
        }
    });
  }, []);

  const profiles = settings?.transcription_profiles ?? [];
  const activeProfileId = settings?.active_profile_id ?? "default";
  const effectiveProfileId = selectedProfileId ?? activeProfileId;

  useEffect(() => {
    if (!settings) return;

    if (!selectedProfileId) {
      setSelectedProfileId(activeProfileId);
      return;
    }

    if (
      selectedProfileId !== "default" &&
      !settings.transcription_profiles?.some(
        (profile) => profile.id === selectedProfileId,
      )
    ) {
      setSelectedProfileId(activeProfileId);
    }
  }, [settings, selectedProfileId, activeProfileId, setSelectedProfileId]);

  const profileOptions = useMemo(
    () => [
      { value: "default", label: t("transcribeFile.defaultProfile") },
      ...profiles.map((profile) => ({
        value: profile.id,
        label: profile.name,
      })),
    ],
    [profiles, t],
  );
  const savedSpeakerNameProfiles = useMemo<SpeakerNameSetProfile[]>(
    () =>
      (settings?.diarization_speaker_name_profiles ?? []).map((profile) => ({
        id: String(profile.id ?? ""),
        name: String(profile.name ?? ""),
        speaker_names: Array.isArray(profile.speaker_names)
          ? profile.speaker_names.map((speakerName) => String(speakerName ?? ""))
          : [],
      })),
    [settings],
  );
  const selectedSpeakerNameProfile = useMemo(
    () =>
      savedSpeakerNameProfiles.find(
        (profile) => profile.id === selectedSpeakerNameProfileId,
      ) ?? null,
    [savedSpeakerNameProfiles, selectedSpeakerNameProfileId],
  );
  const speakerNameProfileOptions = useMemo(
    () =>
      savedSpeakerNameProfiles.map((profile) => ({
        value: profile.id,
        label: profile.name,
      })),
    [savedSpeakerNameProfiles],
  );
  const currentSpeakerNamesForProfile = useMemo(
    () => speakerCards.map((card) => card.name.trim()),
    [speakerCards],
  );
  const canPersistSpeakerNameProfile =
    speakerCards.length > 0 &&
    speakerNameProfileDraftName.trim().length > 0 &&
    currentSpeakerNamesForProfile.some((speakerName) => speakerName.length > 0);
  const canApplySpeakerNameProfile =
    !!selectedSpeakerNameProfile && speakerCards.length > 0;
  const canUpdateSpeakerNameProfile =
    !!selectedSpeakerNameProfile && canPersistSpeakerNameProfile;

  useEffect(() => {
    if (!selectedSpeakerNameProfileId) {
      return;
    }

    if (
      !savedSpeakerNameProfiles.some(
        (profile) => profile.id === selectedSpeakerNameProfileId,
      )
    ) {
      setSelectedSpeakerNameProfileId(null);
      setSpeakerNameProfileDraftName("");
    }
  }, [savedSpeakerNameProfiles, selectedSpeakerNameProfileId]);

  useEffect(() => {
    if (selectedSpeakerNameProfile) {
      setSpeakerNameProfileDraftName(selectedSpeakerNameProfile.name);
    }
  }, [selectedSpeakerNameProfile]);

  const buildSpeakerNameProfileNames = () => {
    const normalized = speakerCards.map((card) => card.name.trim());
    let lastNonEmptyIndex = normalized.length - 1;

    while (lastNonEmptyIndex >= 0 && !normalized[lastNonEmptyIndex]) {
      lastNonEmptyIndex -= 1;
    }

    return normalized.slice(0, lastNonEmptyIndex + 1);
  };

  const persistSpeakerNameProfiles = async (
    nextProfiles: SpeakerNameSetProfile[],
  ) => {
    const result = await commands.changeDiarizationSpeakerNameProfilesSetting(
      nextProfiles.map((profile) => ({
        id: profile.id,
        name: profile.name.trim(),
        speaker_names: profile.speaker_names,
      })),
    );
    if (result.status === "error") {
      throw new Error(result.error);
    }
    await refreshSettings();
  };

  const handleApplySpeakerNameProfile = () => {
    if (!selectedSpeakerNameProfile) {
      return;
    }

    applySpeakerCardNames(selectedSpeakerNameProfile.speaker_names);
  };

  const handleSaveSpeakerNameProfile = async () => {
    if (!canPersistSpeakerNameProfile) {
      return;
    }

    setIsSavingSpeakerNameProfile(true);
    setError(null);

    try {
      const nextProfile: SpeakerNameSetProfile = {
        id: `speaker_names_${Date.now()}_${Math.random()
          .toString(36)
          .slice(2, 8)}`,
        name: speakerNameProfileDraftName.trim(),
        speaker_names: buildSpeakerNameProfileNames(),
      };

      await persistSpeakerNameProfiles([...savedSpeakerNameProfiles, nextProfile]);
      setSelectedSpeakerNameProfileId(nextProfile.id);
      setSpeakerNameProfileDraftName(nextProfile.name);
    } catch (error) {
      setError(String(error));
    } finally {
      setIsSavingSpeakerNameProfile(false);
    }
  };

  const handleUpdateSpeakerNameProfile = async () => {
    if (!selectedSpeakerNameProfile || !canPersistSpeakerNameProfile) {
      return;
    }

    setIsSavingSpeakerNameProfile(true);
    setError(null);

    try {
      const updatedProfile: SpeakerNameSetProfile = {
        ...selectedSpeakerNameProfile,
        name: speakerNameProfileDraftName.trim(),
        speaker_names: buildSpeakerNameProfileNames(),
      };

      await persistSpeakerNameProfiles(
        savedSpeakerNameProfiles.map((profile) =>
          profile.id === updatedProfile.id ? updatedProfile : profile,
        ),
      );
      setSpeakerNameProfileDraftName(updatedProfile.name);
    } catch (error) {
      setError(String(error));
    } finally {
      setIsSavingSpeakerNameProfile(false);
    }
  };

  const handleDeleteSpeakerNameProfile = async () => {
    if (!selectedSpeakerNameProfile) {
      return;
    }

    const confirmed = window.confirm(
      t("transcribeFile.speakerNames.profileDeleteConfirm", {
        name: selectedSpeakerNameProfile.name,
      }),
    );
    if (!confirmed) {
      return;
    }

    setIsSavingSpeakerNameProfile(true);
    setError(null);

    try {
      await persistSpeakerNameProfiles(
        savedSpeakerNameProfiles.filter(
          (profile) => profile.id !== selectedSpeakerNameProfile.id,
        ),
      );
      setSelectedSpeakerNameProfileId(null);
      setSpeakerNameProfileDraftName("");
    } catch (error) {
      setError(String(error));
    } finally {
      setIsSavingSpeakerNameProfile(false);
    }
  };

  // Handle file selection via Tauri dialog
  const handleSelectFile = async () => {
    try {
      const result = await open({
        multiple: false,
        filters: [
          {
            name: "Audio Files",
            extensions: supportedExtensions,
          },
        ],
      });

      if (result) {
        const path = result as string;
        const name = path.split(/[/\\]/).pop() ?? "unknown";
        
        // Get file size
        let fileSize = 0;
        try {
          const fileInfo = await stat(path);
          fileSize = fileInfo.size;
        } catch (e) {
          console.error("Failed to get file size:", e);
        }
        
        setSelectedFile({
          path,
          name,
          size: fileSize,
          audioUrl: convertFileSrc(path),
        });
        setTranscriptionResult("");
        setSavedFilePath(null);
        setInfoMessage(null);
        setError(null);
        clearSpeakerSession();
      }
    } catch (err) {
      console.error("Failed to open file dialog:", err);
      setError(String(err));
    }
  };

  // Transcribe the selected file
  const handleTranscribe = async () => {
    if (!selectedFile) return;

    setIsTranscribing(true);
    setError(null);
    setTranscriptionResult("");
    setSavedFilePath(null);
    setInfoMessage(null);
    clearSpeakerSession();

    try {
      let sonioxOptionsOverride: SonioxFileTranscriptionOptions | null = null;
      if (showSonioxFileOptions) {
        const parsedHints = parseAndNormalizeSonioxLanguageHints(
          sonioxLanguageHintsInput,
        );
        if (parsedHints.rejected.length > 0) {
          setError(
            t("transcribeFile.soniox.invalidHints", {
              hints: parsedHints.rejected.join(", "),
            }),
          );
          setIsTranscribing(false);
          return;
        }

        sonioxOptionsOverride = {
          languageHints:
            parsedHints.normalized.length > 0 ? parsedHints.normalized : null,
          enableSpeakerDiarization: sonioxEnableSpeakerDiarization,
          enableLanguageIdentification: sonioxEnableLanguageIdentification,
        };
      }
      let deepgramOptionsOverride: DeepgramFileTranscriptionOptions | null = null;
      if (showDeepgramFileOptions) {
        deepgramOptionsOverride = {
          diarize: deepgramFileDiarize,
          multichannel: deepgramFileMultichannel,
        };
      }

      const result = await commands.transcribeAudioFile(
        selectedFile.path,
        effectiveProfileId === "default" ? null : effectiveProfileId,
        outputMode === "file",
        outputFormat,
        overrideModelId,
        customWordsEnabledOverride,
        sonioxOptionsOverride,
        deepgramOptionsOverride,
      );

      if (result.status === "ok") {
        setTranscriptionResult(result.data.text);
        setInfoMessage(result.data.info_message ?? null);
        setSpeakerSession(result.data.speaker_session ?? null);
        if (result.data.saved_file_path) {
          setSavedFilePath(result.data.saved_file_path);
        }
      } else {
        clearSpeakerSession();
        setError(result.error);
      }
    } catch (err) {
      clearSpeakerSession();
      setError(String(err));
    } finally {
      setIsTranscribing(false);
    }
  };

  const handleReapplySpeakerNames = async () => {
    if (!speakerArtifactPath || speakerCards.length === 0) {
      setError(
        t(
          "transcribeFile.speakerNames.missingSession",
          "The temporary speaker session is no longer available. Run transcription again.",
        ),
      );
      return;
    }

    if (savedFilePath) {
      setError(
        t(
          "transcribeFile.speakerNames.savedDisabled",
          "Speaker names can only be re-applied before saving to a .txt file.",
        ),
      );
      return;
    }

    setIsReapplyingSpeakerNames(true);
    setError(null);

    try {
      const result = await commands.reapplyTranscriptionSpeakerNames(
        speakerArtifactPath,
        speakerCards.map((card) => ({
          speaker_id: card.speakerId,
          name: card.name,
        })),
      );

      if (result.status === "ok") {
        setTranscriptionResult(result.data);
      } else {
        setError(result.error);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setIsReapplyingSpeakerNames(false);
    }
  };

  // Copy result to clipboard
  const handleCopy = async () => {
    if (!transcriptionResult) return;

    try {
      await navigator.clipboard.writeText(transcriptionResult);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  };

  // Clear selection and results
  const handleClear = () => {
    setSelectedFile(null);
    setTranscriptionResult("");
    setSavedFilePath(null);
    setInfoMessage(null);
    setError(null);
    clearSpeakerSession();
  };

  // Format file size
  const formatFileSize = (bytes: number): string => {
    if (bytes === 0) return "";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6 pb-12">
      {/* Help Section */}
      <SettingsGroup
        title={t("transcribeFile.title")}
        description={t("transcribeFile.description")}
      >
        {/* Drop Zone / File Selection */}
        <div className="px-4 py-4">
          
          {!selectedFile ? (
            <div
              ref={dropZoneRef}
              onClick={handleSelectFile}
              className={`
                border-2 border-dashed rounded-xl p-8 text-center cursor-pointer
                transition-all duration-200
                ${
                  isDragOver
                    ? "border-[#9b5de5] bg-[#9b5de5]/10"
                    : "border-[#333333] hover:border-[#9b5de5]/50 hover:bg-[#1a1a1a]/50"
                }
              `}
            >
              <div className="flex flex-col items-center gap-3">
                <div className={`p-3 rounded-full ${isDragOver ? "bg-[#9b5de5]/20" : "bg-[#1a1a1a]"}`}>
                  <Upload
                    className={`w-8 h-8 ${isDragOver ? "text-[#9b5de5]" : "text-[#b8b8b8]"}`}
                  />
                </div>
                <div>
                  <p className="text-sm font-medium text-[#f5f5f5]">
                    {t("transcribeFile.dropZone.title")}
                  </p>
                  <p className="text-xs text-[#808080] mt-1">
                    {t("transcribeFile.dropZone.subtitle")}
                  </p>
                </div>
                <p className="text-xs text-[#606060]">
                  {t("transcribeFile.dropZone.formats")}
                </p>
              </div>
            </div>
          ) : (
            <div className="space-y-4">
              {/* File Info Card */}
              <div className="flex items-center gap-3 p-3 bg-[#1a1a1a] rounded-lg border border-[#333333]">
                <div className="p-2 bg-[#9b5de5]/20 rounded-lg">
                  <FileAudio className="w-5 h-5 text-[#9b5de5]" />
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium text-[#f5f5f5] truncate">
                    {selectedFile.name}
                  </p>
                  {selectedFile.size > 0 && (
                    <p className="text-xs text-[#808080]">
                      {formatFileSize(selectedFile.size)}
                    </p>
                  )}
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleClear}
                  title={t("transcribeFile.clear")}
                >
                  <Trash2 className="w-4 h-4" />
                </Button>
              </div>

              {/* Audio Preview */}
              <AudioPlayer src={selectedFile.audioUrl} className="w-full" />

              {/* Profile Selector */}
              <div className="space-y-2">
                <label className="text-xs text-[#808080]">
                  {t("transcribeFile.profileLabel")}
                </label>
                <Dropdown
                  className="w-full"
                  selectedValue={effectiveProfileId}
                  options={profileOptions}
                  onSelect={(value) => setSelectedProfileId(value)}
                />
              </div>
            </div>
          )}
        </div>

        {/* Output Mode Selection */}
        {selectedFile && (
          <div className="px-4 py-3 border-t border-white/[0.05]">
            <div className="flex items-center gap-4">
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="radio"
                  name="outputMode"
                  value="textarea"
                  checked={outputMode === "textarea"}
                  onChange={() => setOutputMode("textarea")}
                  className="accent-[#9b5de5]"
                />
                <span className="text-sm text-[#f5f5f5]">
                  {t("transcribeFile.outputMode.textarea")}
                </span>
              </label>
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="radio"
                  name="outputMode"
                  value="file"
                  checked={outputMode === "file"}
                  onChange={() => setOutputMode("file")}
                  className="accent-[#9b5de5]"
                />
                <span className="text-sm text-[#f5f5f5]">
                  {t("transcribeFile.outputMode.file")}
                </span>
              </label>
            </div>
            {/* Output Format Selection */}
            <div className="flex items-center gap-3 mt-3">
              <span className="text-sm text-[#808080]">
                {t("transcribeFile.outputFormat.label")}
              </span>
              <div className="flex gap-2">
                {(["text", "srt", "vtt"] as const).map((fmt) => (
                  <button
                    key={fmt}
                    onClick={() => {
                        setOutputFormat(fmt);
                        // Make sure we have a model selected if switching to subtitle format
                        if (fmt !== 'text' && !overrideModelId && availableModels.length > 0) {
                             const current = availableModels.find(m => m.id === settings?.selected_model);
                             setOverrideModelId(current ? current.id : availableModels[0].id);
                        }
                    }}
                    className={`px-3 py-1 text-xs font-medium rounded transition-all ${
                      outputFormat === fmt
                        ? "bg-[#9b5de5] text-white"
                        : "bg-[#1a1a1a] text-[#b8b8b8] hover:bg-[#222222] border border-[#333333]"
                    }`}
                  >
                    {fmt.toUpperCase()}
                  </button>
                ))}
              </div>
            </div>
            <p className="mt-2 text-xs text-[#606060]">
              {t(
                "transcribeFile.outputFormat.hint",
                "Accurate timestamps (SRT/VTT) require a local model. Remote STT returns text-only output in this version.",
              )}
            </p>
            {showSonioxFileOptions &&
              sonioxModel.trim() !== "stt-async-v4" &&
              !infoMessage && (
              <div className="mt-3 rounded-lg border border-[#9b5de5]/40 bg-[#9b5de5]/10 p-3">
                <p className="text-xs text-[#d7b9ff]">
                  {t("transcribeFile.soniox.autoSwitchNotice", {
                    targetModel: "stt-async-v4",
                    selectedModel: sonioxModel.trim() || "(empty)",
                  })}
                </p>
              </div>
            )}

            {/* Custom Words Toggle */}
            <div className="mt-4 space-y-2">
              <label className="flex items-center gap-2 cursor-pointer select-none">
                <input
                  type="checkbox"
                  checked={customWordsEnabledOverride}
                  onChange={(e) =>
                    setCustomWordsEnabledOverride(e.target.checked)
                  }
                  className="accent-[#9b5de5] w-4 h-4 rounded border-[#333333] bg-[#1a1a1a]"
                />
                <span className="text-sm text-[#f5f5f5]">
                  {t(
                    "transcribeFile.customWords.label",
                    "Apply Custom Words",
                  )}
                </span>
              </label>
              <p className="text-xs text-[#606060] pl-6">
                {t(
                  "transcribeFile.customWords.hint",
                  "Applies your Custom Words list to this file transcription only.",
                )}
              </p>
            </div>

            {showSonioxFileOptions && (
              <div className="mt-4 space-y-3 rounded-lg border border-[#333333] bg-[#151515] p-3">
                <p className="text-sm text-[#f5f5f5]">
                  {t("transcribeFile.soniox.title")}
                </p>
                <p className="text-xs text-[#808080]">
                  {t("transcribeFile.soniox.usesGlobalDefaults")}
                </p>
                <div className="space-y-2">
                  <label className="text-xs text-[#808080]">
                    {t("transcribeFile.soniox.languageHintsLabel")}
                  </label>
                  <input
                    type="text"
                    value={sonioxLanguageHintsInput}
                    onChange={(event) =>
                      setSonioxLanguageHintsInput(event.target.value)
                    }
                    className="w-full rounded border border-[#333333] bg-[#0f0f0f] px-3 py-2 text-sm text-[#f5f5f5] focus:border-[#9b5de5] focus:outline-none"
                    placeholder={t("transcribeFile.soniox.languageHintsPlaceholder")}
                  />
                </div>
                <label className="flex items-center gap-2 cursor-pointer select-none">
                  <input
                    type="checkbox"
                    checked={sonioxEnableSpeakerDiarization}
                    onChange={(e) =>
                      setSonioxEnableSpeakerDiarization(e.target.checked)
                    }
                    className="accent-[#9b5de5] w-4 h-4 rounded border-[#333333] bg-[#1a1a1a]"
                  />
                  <span className="text-sm text-[#f5f5f5]">
                    {t("transcribeFile.soniox.speakerDiarizationLabel")}
                  </span>
                </label>
                <label className="flex items-center gap-2 cursor-pointer select-none">
                  <input
                    type="checkbox"
                    checked={sonioxEnableLanguageIdentification}
                    onChange={(e) =>
                      setSonioxEnableLanguageIdentification(e.target.checked)
                    }
                    className="accent-[#9b5de5] w-4 h-4 rounded border-[#333333] bg-[#1a1a1a]"
                  />
                  <span className="text-sm text-[#f5f5f5]">
                    {t("transcribeFile.soniox.languageIdentificationLabel")}
                  </span>
                </label>
              </div>
            )}

            {showDeepgramFileOptions && (
              <div className="mt-4 space-y-3 rounded-lg border border-[#333333] bg-[#151515] p-3">
                <p className="text-sm text-[#f5f5f5]">
                  {t("transcribeFile.deepgram.title")}
                </p>
                <p className="text-xs text-[#808080]">
                  {t("transcribeFile.deepgram.usesGlobalDefaults")}
                </p>
                <p className="text-xs text-[#606060]">
                  {t("transcribeFile.deepgram.modeHint")}
                </p>
                <label className="flex items-center gap-2 cursor-pointer select-none">
                  <input
                    type="checkbox"
                    checked={deepgramFileDiarize}
                    onChange={(e) => setDeepgramFileDiarize(e.target.checked)}
                    className="accent-[#9b5de5] w-4 h-4 rounded border-[#333333] bg-[#1a1a1a]"
                  />
                  <span className="text-sm text-[#f5f5f5]">
                    {t("transcribeFile.deepgram.speakerDiarizationLabel")}
                  </span>
                </label>
                <label className="flex items-center gap-2 cursor-pointer select-none">
                  <input
                    type="checkbox"
                    checked={deepgramFileMultichannel}
                    onChange={(e) => setDeepgramFileMultichannel(e.target.checked)}
                    className="accent-[#9b5de5] w-4 h-4 rounded border-[#333333] bg-[#1a1a1a]"
                  />
                  <span className="text-sm text-[#f5f5f5]">
                    {t("transcribeFile.deepgram.multichannelLabel")}
                  </span>
                </label>
              </div>
            )}

            {/* Override Model Option */}
            <div className="mt-4 space-y-3">
                <label className="flex items-center gap-2 cursor-pointer select-none">
                    <input 
                        type="checkbox"
                        checked={!!overrideModelId}
                        onChange={(e) => {
                            if (e.target.checked) {
                                // Default to currently selected model if available, or first available
                                const current = availableModels.find(m => m.id === settings?.selected_model);
                                setOverrideModelId(current ? current.id : (availableModels[0]?.id ?? null));
                            } else {
                                setOverrideModelId(null);
                            }
                        }}
                        className="accent-[#9b5de5] w-4 h-4 rounded border-[#333333] bg-[#1a1a1a]" 
                    />
                    <span className="text-sm text-[#f5f5f5]">
                            {t("transcribeFile.modelOverride.label", "Override Model")}
                    </span>
                </label>

                {overrideModelId && (
                    <div className="pl-6">
                        <Dropdown 
                            className="w-full"
                            selectedValue={overrideModelId}
                            options={availableModels.map(m => ({ value: m.id, label: m.name }))}
                            onSelect={setOverrideModelId}
                            placeholder={t("transcribeFile.modelOverride.placeholder", "Select a model...")}
                        />
                         <p className="text-xs text-[#606060] mt-1.5">
                            {t("transcribeFile.modelOverride.hint", "Select a specific local model for this transcription. Local models support accurate timestamping for SRT/VTT.")}
                        </p>
                    </div>
                )}
            </div>
          </div>
        )}

        {/* Action Buttons */}
        {selectedFile && (
          <div className="px-4 py-3 border-t border-white/[0.05]">
            <div className="flex gap-3">
              <Button
                variant="primary"
                onClick={handleTranscribe}
                disabled={isTranscribing || isRecording}
                className="flex items-center gap-2"
                title={isRecording ? t("transcribeFile.recordingInProgress") : undefined}
              >
                {isTranscribing ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    {t("transcribeFile.transcribing")}
                  </>
                ) : (
                  t("transcribeFile.transcribe")
                )}
              </Button>
              <Button variant="secondary" onClick={handleClear}>
                {t("transcribeFile.clear")}
              </Button>
            </div>
            {isRecording && (
              <p className="text-xs text-amber-400 mt-2">
                {t("transcribeFile.recordingInProgress")}
              </p>
            )}
          </div>
        )}

        {/* Error Display */}
        {error && (
          <div className="px-4 py-3 border-t border-white/[0.05]">
            <div className="p-3 bg-red-500/10 border border-red-500/30 rounded-lg">
              <p className="text-sm text-red-400">{error}</p>
            </div>
          </div>
        )}
        {infoMessage && (
          <div className="px-4 py-3 border-t border-white/[0.05]">
            <div className="p-3 bg-[#9b5de5]/10 border border-[#9b5de5]/30 rounded-lg">
              <p className="text-sm text-[#d7b9ff]">{infoMessage}</p>
            </div>
          </div>
        )}

        {/* Results */}
        {transcriptionResult && (
          <div className="px-4 py-3 border-t border-white/[0.05]">
            <div className="space-y-2">
              {speakerCards.length > 0 && (
                <div className="mb-3 rounded-lg border border-[#333333] bg-[#151515] p-3">
                  <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                    <div>
                      <p className="text-sm font-medium text-[#f5f5f5]">
                        {t("transcribeFile.speakerNames.title", "Speaker Names")}
                        {speakerProviderLabel ? (
                          <span className="ml-2 text-xs font-normal text-[#808080]">
                            {speakerProviderLabel}
                          </span>
                        ) : null}
                      </p>
                      <p className="text-xs text-[#808080]">
                        {t(
                          "transcribeFile.speakerNames.hint",
                          "Re-apply rebuilds the transcript from the temporary diarization session instead of editing the visible text.",
                        )}
                      </p>
                    </div>
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={handleReapplySpeakerNames}
                      disabled={!canReapplySpeakerNames || isReapplyingSpeakerNames}
                    >
                      {isReapplyingSpeakerNames ? (
                        <>
                          <Loader2 className="mr-2 h-3 w-3 animate-spin" />
                          {t("transcribeFile.speakerNames.reapplying", "Re-applying...")}
                        </>
                      ) : (
                        t("transcribeFile.speakerNames.reapply", "Re-apply")
                      )}
                    </Button>
                  </div>
                  <div className="mt-3 rounded-lg border border-[#333333] bg-[#101010] p-3">
                    <div className="space-y-3">
                      <div>
                        <p className="text-xs uppercase tracking-wide text-[#808080]">
                          {t("transcribeFile.speakerNames.profilesTitle")}
                        </p>
                        <p className="mt-1 text-xs text-[#606060]">
                          {t("transcribeFile.speakerNames.profilesHint")}
                        </p>
                      </div>
                      <div className="grid gap-3 lg:grid-cols-[minmax(0,1.1fr)_minmax(0,1fr)]">
                        <div className="space-y-2">
                          <label className="text-xs text-[#808080]">
                            {t("transcribeFile.speakerNames.savedProfiles")}
                          </label>
                          <Dropdown
                            className="w-full"
                            selectedValue={selectedSpeakerNameProfileId}
                            options={speakerNameProfileOptions}
                            onSelect={setSelectedSpeakerNameProfileId}
                            placeholder={t(
                              "transcribeFile.speakerNames.profilePlaceholder",
                            )}
                            disabled={
                              speakerNameProfileOptions.length === 0 ||
                              isSavingSpeakerNameProfile
                            }
                          />
                        </div>
                        <div className="space-y-2">
                          <label className="text-xs text-[#808080]">
                            {t("transcribeFile.speakerNames.profileNameLabel")}
                          </label>
                          <input
                            type="text"
                            value={speakerNameProfileDraftName}
                            onChange={(event) =>
                              setSpeakerNameProfileDraftName(event.target.value)
                            }
                            className="w-full rounded border border-[#333333] bg-[#0f0f0f] px-3 py-2 text-sm text-[#f5f5f5] focus:border-[#9b5de5] focus:outline-none"
                            placeholder={t(
                              "transcribeFile.speakerNames.profileNamePlaceholder",
                            )}
                            disabled={isSavingSpeakerNameProfile}
                          />
                        </div>
                      </div>
                      <div className="flex flex-wrap gap-2">
                        <Button
                          variant="secondary"
                          size="sm"
                          onClick={handleApplySpeakerNameProfile}
                          disabled={
                            !canApplySpeakerNameProfile || isSavingSpeakerNameProfile
                          }
                        >
                          {t("transcribeFile.speakerNames.applyProfile")}
                        </Button>
                        <Button
                          variant="secondary"
                          size="sm"
                          onClick={handleSaveSpeakerNameProfile}
                          disabled={
                            !canPersistSpeakerNameProfile ||
                            isSavingSpeakerNameProfile
                          }
                        >
                          {t("transcribeFile.speakerNames.saveProfile")}
                        </Button>
                        <Button
                          variant="secondary"
                          size="sm"
                          onClick={handleUpdateSpeakerNameProfile}
                          disabled={
                            !canUpdateSpeakerNameProfile ||
                            isSavingSpeakerNameProfile
                          }
                        >
                          {t("transcribeFile.speakerNames.updateProfile")}
                        </Button>
                        <Button
                          variant="danger"
                          size="sm"
                          onClick={handleDeleteSpeakerNameProfile}
                          disabled={
                            !selectedSpeakerNameProfile ||
                            isSavingSpeakerNameProfile
                          }
                        >
                          {t("transcribeFile.speakerNames.deleteProfile")}
                        </Button>
                      </div>
                    </div>
                  </div>
                  <div className="mt-3 grid gap-3 sm:grid-cols-2">
                    {speakerCards.map((card) => (
                      <div
                        key={card.speakerId}
                        className="rounded-lg border border-[#333333] bg-[#101010] p-3"
                      >
                        <p className="text-xs uppercase tracking-wide text-[#808080]">
                          {t("transcribeFile.speakerNames.cardLabel", {
                            id: card.speakerId,
                            defaultValue: "Detected speaker {{id}}",
                          })}
                        </p>
                        <input
                          type="text"
                          value={card.name}
                          onChange={(event) =>
                            updateSpeakerCardName(card.speakerId, event.target.value)
                          }
                          className="mt-2 w-full rounded border border-[#333333] bg-[#0f0f0f] px-3 py-2 text-sm text-[#f5f5f5] focus:border-[#9b5de5] focus:outline-none"
                          placeholder={card.defaultName}
                        />
                      </div>
                    ))}
                  </div>
                  {!canReapplySpeakerNames && savedFilePath && (
                    <p className="mt-3 text-xs text-[#b8b8b8]">
                      {t(
                        "transcribeFile.speakerNames.savedDisabled",
                        "Speaker names can only be re-applied before saving to a .txt file.",
                      )}
                    </p>
                  )}
                </div>
              )}
              <div className="flex items-center justify-between">
                <p className="text-xs font-medium text-[#808080] uppercase tracking-wide">
                  {t("transcribeFile.result")}
                </p>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleCopy}
                  className="flex items-center gap-1"
                >
                  {copied ? (
                    <>
                      <Check className="w-3 h-3" />
                      {t("transcribeFile.copied")}
                    </>
                  ) : (
                    <>
                      <Copy className="w-3 h-3" />
                      {t("transcribeFile.copy")}
                    </>
                  )}
                </Button>
              </div>
              <textarea
                readOnly
                value={transcriptionResult}
                className="w-full h-40 p-3 bg-[#0f0f0f] border border-[#333333] rounded-lg text-sm text-[#f5f5f5] resize-none focus:outline-none focus:border-[#9b5de5]"
              />
              {savedFilePath && (
                <div className="flex items-center gap-2 p-2 bg-green-500/10 border border-green-500/30 rounded-lg">
                  <FileText className="w-4 h-4 text-green-400" />
                  <p className="text-xs text-green-400">
                    {t("transcribeFile.savedTo")}: {savedFilePath}
                  </p>
                </div>
              )}
            </div>
          </div>
        )}
      </SettingsGroup>
    </div>
  );
};

export default TranscribeFileSettings;
