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
import { commands, ModelInfo } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { Button } from "@/components/ui/Button";
import { AudioPlayer } from "@/components/ui/AudioPlayer";
import { Dropdown } from "@/components/ui/Dropdown";
import { useTranscribeFileStore } from "@/stores/transcribeFileStore";

const supportedExtensions = ["wav", "mp3", "m4a", "ogg", "flac", "webm"];

export const TranscribeFileSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings } = useSettings();

  const {
    selectedFile,
    outputMode,
    outputFormat,
    overrideModelId,
    transcriptionResult,
    savedFilePath,
    isTranscribing,
    error,
    selectedProfileId,
    setSelectedFile,
    setOutputMode,
    setOutputFormat,
    setOverrideModelId,
    setTranscriptionResult,
    setSavedFilePath,
    setIsTranscribing,
    setError,
    setSelectedProfileId,
  } = useTranscribeFileStore();
  const [isRecording, setIsRecording] = useState(false);
  const [copied, setCopied] = useState(false);
  const [isDragOver, setIsDragOver] = useState(false);
  const [availableModels, setAvailableModels] = useState<ModelInfo[]>([]);

  const dropZoneRef = useRef<HTMLDivElement>(null);

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
          setError(null);
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
        setError(null);
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

    try {
      const result = await commands.transcribeAudioFile(
        selectedFile.path,
        effectiveProfileId === "default" ? null : effectiveProfileId,
        outputMode === "file",
        outputFormat,
        overrideModelId,
      );

      if (result.status === "ok") {
        setTranscriptionResult(result.data.text);
        if (result.data.saved_file_path) {
          setSavedFilePath(result.data.saved_file_path);
        }
      } else {
        setError(result.error);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setIsTranscribing(false);
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
    setError(null);
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

        {/* Results */}
        {transcriptionResult && (
          <div className="px-4 py-3 border-t border-white/[0.05]">
            <div className="space-y-2">
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
