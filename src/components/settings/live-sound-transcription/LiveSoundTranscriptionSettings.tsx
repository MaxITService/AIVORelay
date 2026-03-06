import React, { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { Button } from "../../ui/Button";
import { Dropdown } from "../../ui/Dropdown";
import { SettingContainer } from "../../ui/SettingContainer";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { Textarea } from "../../ui/Textarea";
import { useSettings } from "../../../hooks/useSettings";
import { MicrophoneSelector } from "../MicrophoneSelector";
import { OutputDeviceSelector } from "../OutputDeviceSelector";

type PreviewPayload = {
  final_text?: string;
  interim_text?: string;
  finalText?: string;
  interimText?: string;
};

type LiveSoundState = {
  active?: boolean;
  recording?: boolean;
  processing_llm?: boolean;
  processingLlm?: boolean;
  binding_id?: string | null;
  bindingId?: string | null;
  error_message?: string | null;
  errorMessage?: string | null;
};

const LIVE_SOUND_BINDING_ID = "live_sound_transcription";

const getFinalText = (payload: PreviewPayload) =>
  String(payload.final_text ?? payload.finalText ?? "");

const getInterimText = (payload: PreviewPayload) =>
  String(payload.interim_text ?? payload.interimText ?? "");

const getRecording = (state: LiveSoundState) => Boolean(state.recording);

const getProcessing = (state: LiveSoundState) =>
  Boolean(state.processing_llm ?? state.processingLlm);

const getBindingId = (state: LiveSoundState) =>
  state.binding_id ?? state.bindingId ?? null;

const getErrorMessage = (state: LiveSoundState) =>
  state.error_message ?? state.errorMessage ?? null;

export const LiveSoundTranscriptionSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();
  const [finalText, setFinalText] = useState("");
  const [interimText, setInterimText] = useState("");
  const [isRecording, setIsRecording] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [activeBindingId, setActiveBindingId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [actionBusy, setActionBusy] = useState<null | "start" | "stop" | "clear" | "process">(null);
  const [sourceBusy, setSourceBusy] = useState(false);
  const activeBindingIdRef = useRef<string | null>(null);

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
  const liveSoundCaptureLabel =
    liveSoundCaptureSource === "microphone"
      ? t("settings.liveSoundTranscription.audio.source.options.microphone")
      : t("settings.liveSoundTranscription.audio.source.options.systemOutput");

  const liveProviderReady =
    provider === "remote_soniox" || provider === "remote_deepgram";
  const hasForeignPreviewSession =
    activeBindingId !== null && activeBindingId !== LIVE_SOUND_BINDING_ID;

  const transcriptValue = useMemo(
    () =>
      interimText.trim().length > 0
        ? `${finalText}${finalText && !finalText.endsWith(" ") ? " " : ""}${interimText}`
        : finalText,
    [finalText, interimText],
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
        const bindingId = getBindingId(state);
        activeBindingIdRef.current = bindingId;
        setActiveBindingId(bindingId);
        setIsRecording(getRecording(state));
        setIsProcessing(getProcessing(state));
        setErrorMessage(getErrorMessage(state));
        return state;
      } catch {
        // Ignore polling failures.
        return null;
      }
    };

    const refreshPreview = async () => {
      try {
        const payload = await invoke<PreviewPayload>("get_soniox_live_preview_state");
        if (!active) {
          return;
        }
        if (activeBindingIdRef.current !== LIVE_SOUND_BINDING_ID) {
          return;
        }
        setFinalText(getFinalText(payload));
        setInterimText(getInterimText(payload));
      } catch {
        // Ignore polling failures.
      }
    };

    const setup = async () => {
      unlistenPromises.push(
        listen<PreviewPayload>("soniox-live-preview-update", (event) => {
          if (!active) {
            return;
          }
          if (activeBindingIdRef.current !== LIVE_SOUND_BINDING_ID) {
            return;
          }
          setFinalText(getFinalText(event.payload));
          setInterimText(getInterimText(event.payload));
        }),
      );
      unlistenPromises.push(
        listen<PreviewPayload>("soniox_live_preview_update", (event) => {
          if (!active) {
            return;
          }
          if (activeBindingIdRef.current !== LIVE_SOUND_BINDING_ID) {
            return;
          }
          setFinalText(getFinalText(event.payload));
          setInterimText(getInterimText(event.payload));
        }),
      );
      unlistenPromises.push(
        listen("soniox-live-preview-reset", () => {
          if (!active) {
            return;
          }
          if (activeBindingIdRef.current !== LIVE_SOUND_BINDING_ID) {
            return;
          }
          setFinalText("");
          setInterimText("");
        }),
      );
      unlistenPromises.push(
        listen("soniox_live_preview_reset", () => {
          if (!active) {
            return;
          }
          if (activeBindingIdRef.current !== LIVE_SOUND_BINDING_ID) {
            return;
          }
          setFinalText("");
          setInterimText("");
        }),
      );
      unlistenPromises.push(
        listen<LiveSoundState>("preview-output-mode-state", (event) => {
          if (!active) {
            return;
          }
          const bindingId = getBindingId(event.payload);
          activeBindingIdRef.current = bindingId;
          setActiveBindingId(bindingId);
          setIsRecording(getRecording(event.payload));
          setIsProcessing(getProcessing(event.payload));
          setErrorMessage(getErrorMessage(event.payload));
        }),
      );
      unlistenPromises.push(
        listen<LiveSoundState>("preview_output_mode_state", (event) => {
          if (!active) {
            return;
          }
          const bindingId = getBindingId(event.payload);
          activeBindingIdRef.current = bindingId;
          setActiveBindingId(bindingId);
          setIsRecording(getRecording(event.payload));
          setIsProcessing(getProcessing(event.payload));
          setErrorMessage(getErrorMessage(event.payload));
        }),
      );
    };

    void (async () => {
      await setup();
      const state = await refreshState();
      if (getBindingId(state ?? {}) === LIVE_SOUND_BINDING_ID) {
        await refreshPreview();
      }
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
                  {t("settings.liveSoundTranscription.session.sourceLabel")}
                </p>
                <p className="mt-1 text-sm font-medium text-[#f5f5f5]">
                  {liveSoundCaptureLabel}
                </p>
              </div>
            </div>

            {!liveProviderReady && (
              <div className="rounded-lg border border-amber-500/25 bg-amber-500/10 px-4 py-3 text-sm text-amber-100">
                {t("settings.liveSoundTranscription.session.remoteOnly")}
              </div>
            )}

            {hasForeignPreviewSession && (
              <div className="rounded-lg border border-amber-500/25 bg-amber-500/10 px-4 py-3 text-sm text-amber-100">
                {t("settings.liveSoundTranscription.session.anotherSessionActive")}
              </div>
            )}

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
                  hasForeignPreviewSession ||
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
                  hasForeignPreviewSession ||
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
                  hasForeignPreviewSession ||
                  sourceBusy ||
                  actionBusy !== null ||
                  isProcessing ||
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
            <Textarea
              readOnly
              value={transcriptValue}
              placeholder={t("settings.liveSoundTranscription.transcript.empty")}
              className="w-full"
            />
            {interimText.trim().length > 0 && (
              <p className="text-xs text-[#9a9a9a]">
                {t("settings.liveSoundTranscription.transcript.interimActive")}
              </p>
            )}
          </div>
        </SettingContainer>
      </SettingsGroup>

      <SettingsGroup title={t("settings.sound.title")}>
        <SettingContainer
          title={t("settings.liveSoundTranscription.audio.source.title")}
          description={t("settings.liveSoundTranscription.audio.source.description")}
          grouped={true}
        >
          <div className="space-y-3">
            <Dropdown
              options={[
                {
                  value: "system_output",
                  label: t(
                    "settings.liveSoundTranscription.audio.source.options.systemOutput",
                  ),
                },
                {
                  value: "microphone",
                  label: t(
                    "settings.liveSoundTranscription.audio.source.options.microphone",
                  ),
                },
              ]}
              selectedValue={liveSoundCaptureSource}
              onSelect={handleCaptureSourceSelect}
              disabled={sourceBusy || isRecording || actionBusy !== null}
            />
            <p className="text-xs text-[#9a9a9a]">
              {t("settings.liveSoundTranscription.audio.source.hint")}
            </p>
          </div>
        </SettingContainer>
        <MicrophoneSelector
          descriptionMode="tooltip"
          grouped={true}
          disabled={sourceBusy || isRecording || actionBusy !== null}
          descriptionOverride={t(
            "settings.liveSoundTranscription.audio.microphoneDescription",
          )}
        />
        <OutputDeviceSelector
          descriptionMode="tooltip"
          grouped={true}
          disabled={sourceBusy || isRecording || actionBusy !== null}
          descriptionOverride={t(
            "settings.liveSoundTranscription.audio.outputDescription",
          )}
        />
      </SettingsGroup>
    </div>
  );
};
