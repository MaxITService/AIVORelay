import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { Copy, Check, FileText } from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";
import { Button } from "../../ui/Button";
import { SettingContainer } from "../../ui/SettingContainer";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { useSettings } from "../../../hooks/useSettings";
import { MicrophoneSelector } from "../MicrophoneSelector";
import { OutputDeviceSelector } from "../OutputDeviceSelector";

type LiveSoundSegment = {
  speaker_id?: number | null;
  speakerId?: number | null;
  speaker_label?: string | null;
  speakerLabel?: string | null;
  text?: string;
  is_interim?: boolean;
  isInterim?: boolean;
};

type LiveSoundState = {
  active?: boolean;
  recording?: boolean;
  processing_llm?: boolean;
  processingLlm?: boolean;
  error_message?: string | null;
  errorMessage?: string | null;
  final_text?: string;
  finalText?: string;
  interim_text?: string;
  interimText?: string;
  segments?: LiveSoundSegment[];
};

const getRecording = (state: LiveSoundState) => Boolean(state.recording);

const getProcessing = (state: LiveSoundState) =>
  Boolean(state.processing_llm ?? state.processingLlm);

const getErrorMessage = (state: LiveSoundState) =>
  state.error_message ?? state.errorMessage ?? null;

const getFinalText = (state: LiveSoundState) =>
  String(state.final_text ?? state.finalText ?? "");

const getInterimText = (state: LiveSoundState) =>
  String(state.interim_text ?? state.interimText ?? "");

const getSegments = (state: LiveSoundState) =>
  Array.isArray(state.segments) ? state.segments : [];

const getSegmentSpeakerLabel = (segment: LiveSoundSegment) =>
  segment.speaker_label ?? segment.speakerLabel ?? null;

const getSegmentText = (segment: LiveSoundSegment) => String(segment.text ?? "");

const isInterimSegment = (segment: LiveSoundSegment) =>
  Boolean(segment.is_interim ?? segment.isInterim);

export const LiveSoundTranscriptionSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();
  const [finalText, setFinalText] = useState("");
  const [interimText, setInterimText] = useState("");
  const [isRecording, setIsRecording] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [segments, setSegments] = useState<LiveSoundSegment[]>([]);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [actionBusy, setActionBusy] = useState<null | "start" | "stop" | "clear" | "process">(null);
  const [sourceBusy, setSourceBusy] = useState(false);
  const [copied, setCopied] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [speakerNames, setSpeakerNames] = useState<Map<number, string>>(new Map());
  const [editingSpeakerId, setEditingSpeakerId] = useState<number | null>(null);
  const [editValue, setEditValue] = useState("");

  const provider = String(settings?.transcription_provider ?? "local");
  const providerLabel =
    provider === "remote_soniox"
      ? "Soniox"
      : provider === "remote_deepgram"
        ? "Deepgram"
        : provider === "remote_openai_compatible"
          ? "OpenAI-compatible API"
          : t("settings.liveSoundTranscription.session.providerUnsupported");

  const modelLabel =
    provider === "remote_soniox"
      ? String((settings as any)?.soniox_model ?? "stt-rt-v4")
      : provider === "remote_deepgram"
        ? String((settings as any)?.deepgram_model ?? "nova-3")
        : String(
            settings?.selected_model ??
              t("settings.liveSoundTranscription.session.notAvailable"),
          );

  const liveModeEnabled =
    provider === "remote_soniox"
      ? Boolean((settings as any)?.soniox_live_enabled ?? true)
      : provider === "remote_deepgram"
        ? Boolean((settings as any)?.deepgram_live_enabled ?? true)
        : false;
  const liveSoundCaptureSource = String(
    (settings as any)?.live_sound_capture_source ?? "system_output",
  );
  const micEnabled = liveSoundCaptureSource === "microphone" || liveSoundCaptureSource === "both";
  const outputEnabled = liveSoundCaptureSource === "system_output" || liveSoundCaptureSource === "both";

  const liveProviderReady =
    provider === "remote_soniox" || provider === "remote_deepgram";
  const diarizationEnabled = Boolean(
    (settings as any)?.live_sound_enable_speaker_diarization ?? true,
  );
  const finalizedSegments = segments.filter(
    (segment) => !isInterimSegment(segment) && getSegmentText(segment).trim().length > 0,
  );
  const interimSegments = segments.filter(
    (segment) => isInterimSegment(segment) && getSegmentText(segment).trim().length > 0,
  );

  useEffect(() => {
    let active = true;
    const unlistenPromises: Array<Promise<() => void>> = [];

    const refreshState = async () => {
      try {
        const state = await invoke<LiveSoundState>("get_live_sound_transcription_state");
        if (!active) {
          return null;
        }
        setIsRecording(getRecording(state));
        setIsProcessing(getProcessing(state));
        setFinalText(getFinalText(state));
        setInterimText(getInterimText(state));
        setSegments(getSegments(state));
        setErrorMessage(getErrorMessage(state));
        return state;
      } catch {
        // Ignore polling failures.
        return null;
      }
    };

    const setup = async () => {
      unlistenPromises.push(
        listen<LiveSoundState>("live-sound-transcription-state", (event) => {
          if (!active) {
            return;
          }
          setIsRecording(getRecording(event.payload));
          setIsProcessing(getProcessing(event.payload));
          setFinalText(getFinalText(event.payload));
          setInterimText(getInterimText(event.payload));
          setSegments(getSegments(event.payload));
          setErrorMessage(getErrorMessage(event.payload));
        }),
      );
      unlistenPromises.push(
        listen<LiveSoundState>("live_sound_transcription_state", (event) => {
          if (!active) {
            return;
          }
          setIsRecording(getRecording(event.payload));
          setIsProcessing(getProcessing(event.payload));
          setFinalText(getFinalText(event.payload));
          setInterimText(getInterimText(event.payload));
          setSegments(getSegments(event.payload));
          setErrorMessage(getErrorMessage(event.payload));
        }),
      );
    };

    void (async () => {
      await setup();
      await refreshState();
    })();

    return () => {
      active = false;
      for (const unlistenPromise of unlistenPromises) {
        void unlistenPromise.then((unlisten) => unlisten());
      }
    };
  }, []);

  const runAction = async (
    action: "start" | "stop" | "clear" | "process",
    command:
      | "live_sound_transcription_start"
      | "live_sound_transcription_stop"
      | "live_sound_transcription_clear"
      | "live_sound_transcription_process",
  ) => {
    setActionBusy(action);
    setErrorMessage(null);
    if (action === "clear") setSpeakerNames(new Map());
    try {
      await invoke(command);
    } catch (error) {
      setErrorMessage(
        error instanceof Error
          ? error.message
          : typeof error === "string"
            ? error
            : t("settings.liveSoundTranscription.session.actionFailed"),
      );
    } finally {
      setActionBusy(null);
    }
  };

  const handleCaptureSourceSelect = async (source: string) => {
    setSourceBusy(true);
    setErrorMessage(null);
    try {
      await invoke("change_live_sound_capture_source_setting", { source });
      await refreshSettings();
    } catch (error) {
      setErrorMessage(
        error instanceof Error
          ? error.message
          : typeof error === "string"
            ? error
            : t("settings.liveSoundTranscription.session.actionFailed"),
      );
    } finally {
      setSourceBusy(false);
    }
  };

  const handleSourceToggle = async (toggledMic: boolean, toggledOutput: boolean) => {
    // Prevent both being disabled — at least one must stay on.
    if (!toggledMic && !toggledOutput) return;
    const source = toggledMic && toggledOutput ? "both" : toggledMic ? "microphone" : "system_output";
    await handleCaptureSourceSelect(source);
  };

  const handleDiarizationToggle = async () => {
    setSourceBusy(true);
    setErrorMessage(null);
    try {
      await invoke("change_live_sound_speaker_diarization_setting", {
        enabled: !diarizationEnabled,
      });
      await refreshSettings();
    } catch (error) {
      setErrorMessage(
        error instanceof Error
          ? error.message
          : typeof error === "string"
            ? error
            : t("settings.liveSoundTranscription.session.actionFailed"),
      );
    } finally {
      setSourceBusy(false);
    }
  };

  const hasTranscript = finalText.trim().length > 0 || segments.some((s) => !isInterimSegment(s) && (s.text ?? "").trim().length > 0);

  const getDisplayName = (segment: LiveSoundSegment): string | null => {
    const id = segment.speaker_id ?? segment.speakerId;
    if (id != null && speakerNames.has(id)) return speakerNames.get(id)!;
    return getSegmentSpeakerLabel(segment);
  };

  const handleSpeakerClick = (speakerId: number, currentDisplay: string) => {
    setEditingSpeakerId(speakerId);
    setEditValue(speakerNames.get(speakerId) ?? currentDisplay);
  };

  const commitSpeakerName = (speakerId: number) => {
    const trimmed = editValue.trim();
    setSpeakerNames((prev) => {
      const next = new Map(prev);
      if (trimmed) next.set(speakerId, trimmed);
      else next.delete(speakerId);
      return next;
    });
    setEditingSpeakerId(null);
    setEditValue("");
  };

  const buildPlainText = () => {
    const finalSegs = segments.filter((s) => !isInterimSegment(s) && (s.text ?? "").trim().length > 0);
    if (finalSegs.length === 0) return finalText.trim();
    const lines: string[] = [];
    let lastLabel: string | null = null;
    for (const seg of finalSegs) {
      const label = getDisplayName(seg);
      const text = getSegmentText(seg).trim();
      if (label && label !== lastLabel) {
        if (lines.length > 0) lines.push("");
        lines.push(`${label}:`);
        lastLabel = label;
      }
      lines.push(text);
    }
    return lines.join("\n");
  };

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(buildPlainText());
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // ignore
    }
  };

  const handleSaveFile = async () => {
    const path = await save({
      filters: [{ name: "Text", extensions: ["txt"] }],
      defaultPath: `live-transcript-${new Date().toISOString().slice(0, 19).replace(/[T:]/g, "-")}.txt`,
    });
    if (!path) return;
    setIsSaving(true);
    try {
      await invoke("save_live_sound_transcript", { path, content: buildPlainText() });
    } catch (error) {
      setErrorMessage(
        error instanceof Error
          ? error.message
          : typeof error === "string"
            ? error
            : t("settings.liveSoundTranscription.session.actionFailed"),
      );
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-8 pb-12">
      <SettingsGroup
        title={t("settings.liveSoundTranscription.title")}
        description={t("settings.liveSoundTranscription.description")}
      >
        <SettingContainer
          title={t("settings.liveSoundTranscription.session.title")}
          description={t("settings.liveSoundTranscription.session.description")}
          grouped={true}
          layout="stacked"
        >
          <div className="space-y-4">
            <div className="grid gap-3 md:grid-cols-2">
              <div className="rounded-lg border border-[#333333] bg-[#121212]/70 px-4 py-3">
                <p className="text-[11px] uppercase tracking-[0.18em] text-[#8a8a8a]">
                  {t("settings.liveSoundTranscription.session.providerLabel")}
                </p>
                <p className="mt-1 text-sm font-medium text-[#f5f5f5]">
                  {providerLabel}
                </p>
              </div>
              <div className="rounded-lg border border-[#333333] bg-[#121212]/70 px-4 py-3">
                <p className="text-[11px] uppercase tracking-[0.18em] text-[#8a8a8a]">
                  {t("settings.liveSoundTranscription.session.modelLabel")}
                </p>
                <p className="mt-1 text-sm font-medium text-[#f5f5f5]">
                  {modelLabel}
                </p>
              </div>
              <div className="rounded-lg border border-[#333333] bg-[#121212]/70 px-4 py-3">
                <p className="text-[11px] uppercase tracking-[0.18em] text-[#8a8a8a]">
                  {t("settings.liveSoundTranscription.session.liveModeLabel")}
                </p>
                <p className="mt-1 text-sm font-medium text-[#f5f5f5]">
                  {liveModeEnabled
                    ? t("settings.liveSoundTranscription.session.enabled")
                    : t("settings.liveSoundTranscription.session.disabled")}
                </p>
              </div>
              <div className="rounded-lg border border-[#333333] bg-[#121212]/70 px-4 py-3">
                <p className="text-[11px] uppercase tracking-[0.18em] text-[#8a8a8a]">
                  {t("settings.liveSoundTranscription.session.diarizationLabel")}
                </p>
                <p className="mt-1 text-sm font-medium text-[#f5f5f5]">
                  {diarizationEnabled
                    ? t("settings.liveSoundTranscription.session.enabled")
                    : t("settings.liveSoundTranscription.session.disabled")}
                </p>
              </div>
            </div>

            {!liveProviderReady && (
              <div className="rounded-lg border border-amber-500/25 bg-amber-500/10 px-4 py-3 text-sm text-amber-100">
                {t("settings.liveSoundTranscription.session.remoteOnly")}
              </div>
            )}

            {!diarizationEnabled && (
              <div className="rounded-lg border border-amber-500/25 bg-amber-500/10 px-4 py-3 text-sm text-amber-100">
                {t("settings.liveSoundTranscription.session.diarizationDisabledHint")}
              </div>
            )}

            <div className="flex flex-wrap items-center gap-3 rounded-lg border border-[#333333] bg-[#121212]/50 px-4 py-3">
              <div className="min-w-0 flex-1">
                <p className="text-sm font-medium text-[#f5f5f5]">
                  {t("settings.liveSoundTranscription.session.diarizationToggleTitle")}
                </p>
                <p className="text-xs text-[#9a9a9a]">
                  {t("settings.liveSoundTranscription.session.diarizationToggleDescription")}
                </p>
              </div>
              <Button
                variant={diarizationEnabled ? "secondary" : "primary"}
                disabled={sourceBusy || actionBusy !== null || isRecording}
                onClick={() => void handleDiarizationToggle()}
              >
                {diarizationEnabled
                  ? t("settings.liveSoundTranscription.session.diarizationTurnOff")
                  : t("settings.liveSoundTranscription.session.diarizationTurnOn")}
              </Button>
            </div>

            {errorMessage && (
              <div className="rounded-lg border border-[#6b2c2c] bg-[#351616]/80 px-4 py-3 text-sm text-[#ffd4d4]">
                {errorMessage}
              </div>
            )}

            <div className="flex flex-wrap gap-2">
              <Button
                variant="primary"
                disabled={
                  !liveProviderReady ||
                  !liveModeEnabled ||
                  isRecording ||
                  sourceBusy ||
                  actionBusy !== null
                }
                onClick={() =>
                  void runAction("start", "live_sound_transcription_start")
                }
              >
                {actionBusy === "start"
                  ? t("settings.liveSoundTranscription.session.starting")
                  : t("settings.liveSoundTranscription.session.start")}
              </Button>
              <Button
                variant="danger"
                disabled={!isRecording || sourceBusy || actionBusy !== null}
                onClick={() =>
                  void runAction("stop", "live_sound_transcription_stop")
                }
              >
                {actionBusy === "stop"
                  ? t("settings.liveSoundTranscription.session.stopping")
                  : t("settings.liveSoundTranscription.session.stop")}
              </Button>
              <Button
                variant="secondary"
                disabled={
                  sourceBusy ||
                  actionBusy !== null ||
                  (finalText.trim().length === 0 && interimText.trim().length === 0)
                }
                onClick={() =>
                  void runAction("clear", "live_sound_transcription_clear")
                }
              >
                {actionBusy === "clear"
                  ? t("settings.liveSoundTranscription.session.clearing")
                  : t("settings.liveSoundTranscription.session.clear")}
              </Button>
              <Button
                variant="secondary"
                disabled={
                  sourceBusy ||
                  actionBusy !== null ||
                  isProcessing ||
                  isRecording ||
                  finalText.trim().length === 0
                }
                onClick={() =>
                  void runAction("process", "live_sound_transcription_process")
                }
              >
                {actionBusy === "process" || isProcessing
                  ? t("settings.liveSoundTranscription.session.processing")
                  : t("settings.liveSoundTranscription.session.process")}
              </Button>
            </div>

            <p className="text-sm text-[#9a9a9a]">
              {isRecording
                ? t("settings.liveSoundTranscription.session.recordingHint")
                : t("settings.liveSoundTranscription.session.idleHint")}
            </p>
          </div>
        </SettingContainer>
      </SettingsGroup>

      <SettingsGroup
        title={t("settings.liveSoundTranscription.transcript.title")}
        description={t("settings.liveSoundTranscription.transcript.description")}
      >
        <SettingContainer
          title={t("settings.liveSoundTranscription.transcript.previewTitle")}
          description={t("settings.liveSoundTranscription.transcript.previewDescription")}
          grouped={true}
          layout="stacked"
        >
          <div className="space-y-3">
            {segments.length === 0 ? (
              <div className="rounded-lg border border-dashed border-[#333333] bg-[#111111]/60 px-4 py-6 text-sm text-[#8a8a8a]">
                {t("settings.liveSoundTranscription.transcript.empty")}
              </div>
            ) : (
              <div className="space-y-3">
                {finalizedSegments.map((segment, index) => {
                  const speakerId = segment.speaker_id ?? segment.speakerId ?? null;
                  const displayName = getDisplayName(segment);
                  const isEditing = speakerId != null && editingSpeakerId === speakerId;
                  return (
                    <div
                      key={`final-${index}`}
                      className="rounded-lg border border-[#333333] bg-[#121212]/70 px-4 py-3"
                    >
                      {displayName && (
                        isEditing ? (
                          <input
                            autoFocus
                            className="text-[11px] uppercase tracking-[0.18em] text-[#c0c0c0] bg-transparent border-b border-[#555] outline-none w-full mb-0"
                            value={editValue}
                            placeholder={t("settings.liveSoundTranscription.transcript.speakerNamePlaceholder")}
                            onChange={(e) => setEditValue(e.target.value)}
                            onBlur={() => speakerId != null && commitSpeakerName(speakerId)}
                            onKeyDown={(e) => {
                              if (e.key === "Enter" && speakerId != null) commitSpeakerName(speakerId);
                              if (e.key === "Escape") { setEditingSpeakerId(null); setEditValue(""); }
                            }}
                          />
                        ) : (
                          <button
                            className="text-[11px] uppercase tracking-[0.18em] text-[#8a8a8a] hover:text-[#bbbbbb] text-left transition-colors"
                            title={t("settings.liveSoundTranscription.transcript.speakerNameHint")}
                            onClick={() => speakerId != null && handleSpeakerClick(speakerId, displayName)}
                          >
                            {displayName}
                          </button>
                        )
                      )}
                      <p className="mt-1 whitespace-pre-wrap text-sm text-[#f5f5f5]">
                        {getSegmentText(segment)}
                      </p>
                    </div>
                  );
                })}
                {interimSegments.map((segment, index) => (
                  <div
                    key={`interim-${index}`}
                    className="rounded-lg border border-dashed border-[#3a3a3a] bg-[#111111]/60 px-4 py-3"
                  >
                    <p className="text-[11px] uppercase tracking-[0.18em] text-[#8a8a8a]">
                      {getSegmentSpeakerLabel(segment) ??
                        t("settings.liveSoundTranscription.transcript.interimLabel")}
                    </p>
                    <p className="mt-1 whitespace-pre-wrap text-sm text-[#d0d0d0]">
                      {getSegmentText(segment)}
                    </p>
                  </div>
                ))}
              </div>
            )}
            {interimText.trim().length > 0 && (
              <p className="text-xs text-[#9a9a9a]">
                {t("settings.liveSoundTranscription.transcript.interimActive")}
              </p>
            )}
            {hasTranscript && (
              <div className="flex flex-wrap gap-2 pt-1">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => void handleCopy()}
                  className="flex items-center gap-1"
                >
                  {copied ? (
                    <>
                      <Check className="w-3 h-3" />
                      {t("settings.liveSoundTranscription.transcript.copied")}
                    </>
                  ) : (
                    <>
                      <Copy className="w-3 h-3" />
                      {t("settings.liveSoundTranscription.transcript.copy")}
                    </>
                  )}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={isSaving}
                  onClick={() => void handleSaveFile()}
                  className="flex items-center gap-1"
                >
                  <FileText className="w-3 h-3" />
                  {isSaving
                    ? t("settings.liveSoundTranscription.transcript.saving")
                    : t("settings.liveSoundTranscription.transcript.saveFile")}
                </Button>
              </div>
            )}
          </div>
        </SettingContainer>
      </SettingsGroup>

      <SettingsGroup title={t("settings.sound.title")}>
        <div className={micEnabled ? "rounded-xl ring-2 ring-emerald-500/40" : undefined}>
          {micEnabled && (
            <div className="px-4 pt-3 pb-0">
              <span className="inline-flex items-center gap-1.5 rounded-full bg-emerald-500/15 px-2.5 py-1 text-xs font-semibold text-emerald-400">
                <span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
                {t("settings.liveSoundTranscription.audio.listening")}
              </span>
            </div>
          )}
          <MicrophoneSelector
            descriptionMode="tooltip"
            grouped={true}
            disabled={sourceBusy || isRecording || actionBusy !== null}
            titleOverride={t("settings.liveSoundTranscription.audio.microphoneTitle")}
          />
          <SettingContainer
            title={t("settings.liveSoundTranscription.audio.microphoneToggle")}
            description={t("settings.liveSoundTranscription.audio.microphoneToggleDescription")}
            grouped={true}
          >
            <Button
              variant={micEnabled ? "secondary" : "primary"}
              disabled={sourceBusy || isRecording || actionBusy !== null || (micEnabled && !outputEnabled)}
              onClick={() => void handleSourceToggle(!micEnabled, outputEnabled)}
            >
              {micEnabled && !outputEnabled
                ? t("settings.liveSoundTranscription.audio.onlySource")
                : micEnabled
                  ? t("settings.liveSoundTranscription.audio.sourceDisable")
                  : t("settings.liveSoundTranscription.audio.sourceEnable")}
            </Button>
          </SettingContainer>
        </div>
        <div className={outputEnabled ? "rounded-xl ring-2 ring-emerald-500/40" : undefined}>
          {outputEnabled && (
            <div className="px-4 pt-3 pb-0">
              <span className="inline-flex items-center gap-1.5 rounded-full bg-emerald-500/15 px-2.5 py-1 text-xs font-semibold text-emerald-400">
                <span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
                {t("settings.liveSoundTranscription.audio.listening")}
              </span>
            </div>
          )}
          <OutputDeviceSelector
            descriptionMode="tooltip"
            grouped={true}
            disabled={sourceBusy || isRecording || actionBusy !== null}
            titleOverride={t("settings.liveSoundTranscription.audio.outputTitle")}
          />
          <SettingContainer
            title={t("settings.liveSoundTranscription.audio.outputToggle")}
            description={t("settings.liveSoundTranscription.audio.outputToggleDescription")}
            grouped={true}
          >
            <Button
              variant={outputEnabled ? "secondary" : "primary"}
              disabled={sourceBusy || isRecording || actionBusy !== null || (outputEnabled && !micEnabled)}
              onClick={() => void handleSourceToggle(micEnabled, !outputEnabled)}
            >
              {outputEnabled && !micEnabled
                ? t("settings.liveSoundTranscription.audio.onlySource")
                : outputEnabled
                  ? t("settings.liveSoundTranscription.audio.sourceDisable")
                  : t("settings.liveSoundTranscription.audio.sourceEnable")}
            </Button>
          </SettingContainer>
        </div>
      </SettingsGroup>
    </div>
  );
};




