import React, { useMemo, useState } from "react";
import { CheckCircle2, Loader2, Sparkles } from "lucide-react";
import { useTranslation } from "react-i18next";

import { AudioPlayer, AudioPlayerGroup } from "@/components/ui/AudioPlayer";
import { Button } from "@/components/ui/Button";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import type { TtsProvider } from "@/lib/tts/ttsProviderMetadata";
import voiceGalleryManifestJson from "./ttsVoiceGalleryManifest.json";

type TtsVoiceGalleryProviderId = Extract<
  TtsProvider,
  "soniox" | "deepgram" | "openai" | "murf" | "local_qwen" | "local_kokoro"
>;

type TtsVoiceGalleryProvider = {
  label: string;
  kind: "remote" | "local";
  license: {
    kind: "provider-output-terms" | "open-source-license";
    name: string;
    url: string;
    checkedOn: string;
  };
};

type TtsVoiceGalleryManifest = {
  schemaVersion: 1;
  generatedOn: string;
  previewEncoding: {
    container: "ogg";
    codec: "opus";
    bitrateKbps: number;
  };
  providers: Record<TtsVoiceGalleryProviderId, TtsVoiceGalleryProvider>;
  voices: Array<
    Omit<TtsVoiceGalleryEntry, "provider" | "providerLabel" | "asset"> & {
      provider: TtsVoiceGalleryProviderId;
      asset: {
        path: string;
        bytes: number;
        sha256: string;
      };
    }
  >;
};

export type TtsVoiceGalleryEntry = {
  id: string;
  provider: TtsProvider;
  providerLabel: string;
  voiceName: string;
  modelFamily: string;
  model: string;
  voice: string;
  language: string;
  speed: number;
  murfRate?: number;
  murfPitch?: number;
  murfVariation?: number;
  murfStyle?: string;
  voiceInstructions?: string;
  transcript: string;
  asset: {
    path: string;
    bytes: number;
    sha256: string;
  };
};

export const TTS_VOICE_GALLERY_MANIFEST =
  voiceGalleryManifestJson as unknown as TtsVoiceGalleryManifest;

export const TTS_VOICE_GALLERY: TtsVoiceGalleryEntry[] =
  TTS_VOICE_GALLERY_MANIFEST.voices.map((entry) => ({
    ...entry,
    providerLabel:
      TTS_VOICE_GALLERY_MANIFEST.providers[entry.provider].label,
  }));

export const synthesisConfigForGalleryVoice = <
  Config extends { provider: TtsProvider; model: string },
>(
  entry: TtsVoiceGalleryEntry,
  currentConfig: Config,
  savedConfigs: readonly Config[],
) => {
  const remembered = savedConfigs.find(
    (config) =>
      config.provider === entry.provider &&
      config.model.trim() === entry.model.trim(),
  );
  return {
    ...(remembered ?? currentConfig),
    provider: entry.provider,
    model: entry.model,
    voice: entry.voice,
    language: entry.language,
    speed: entry.speed,
    murf_rate: entry.murfRate ?? 0,
    murf_pitch: entry.murfPitch ?? 0,
    murf_variation: entry.murfVariation ?? 1,
    murf_style: entry.murfStyle ?? null,
    voice_instructions: entry.voiceInstructions ?? "",
    voice_prompt_preset_id: "",
  };
};

const assetUrl = (path: string) => `${import.meta.env.BASE_URL}${path}`;

type TtsVoiceGalleryProps = {
  savingSettings: boolean;
  applyingId: string | null;
  appliedId: string | null;
  onApply: (entry: TtsVoiceGalleryEntry) => Promise<void>;
};

export const TtsVoiceGallery: React.FC<TtsVoiceGalleryProps> = ({
  savingSettings,
  applyingId,
  appliedId,
  onApply,
}) => {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = useState(true);
  const providerGroups = useMemo(
    () =>
      TTS_VOICE_GALLERY.reduce<
        Array<{ provider: string; entries: TtsVoiceGalleryEntry[] }>
      >((groups, entry) => {
        const existing = groups.find(
          (group) => group.provider === entry.providerLabel,
        );
        if (existing) existing.entries.push(entry);
        else groups.push({ provider: entry.providerLabel, entries: [entry] });
        return groups;
      }, []),
    [],
  );

  return (
    <SettingsGroup
      id="tts-voice-gallery"
      title={t("textToSpeech.voiceGallery.title")}
      description={t("textToSpeech.voiceGallery.description")}
      collapsible
      collapsed={collapsed}
      onCollapsedChange={setCollapsed}
      collapseLabel={t("textToSpeech.voiceGallery.collapse")}
      expandLabel={t("textToSpeech.voiceGallery.expand")}
      toggleTestId="tts-voice-gallery-toggle"
    >
      <div className="flex items-start gap-3 bg-violet-400/[0.07] px-6 py-4 text-sm text-violet-50/85">
        <Sparkles className="mt-0.5 h-4 w-4 shrink-0 text-violet-200" />
        <p className="leading-relaxed">
          {t("textToSpeech.voiceGallery.bundledNote")}
        </p>
      </div>
      <AudioPlayerGroup>
        <div className="space-y-7 px-4 py-5 sm:px-6">
          {providerGroups.map((group) => (
            <section key={group.provider} aria-label={group.provider}>
              <div className="mb-3 flex items-center gap-3">
                <h3 className="text-xs font-bold uppercase tracking-[0.14em] text-[#d7b9ff]">
                  {group.provider}
                </h3>
                <span className="text-[11px] text-[#777]">
                  {t("textToSpeech.voiceGallery.voiceCount", {
                    count: group.entries.length,
                  })}
                </span>
              </div>
              <div className="grid gap-3 md:grid-cols-2">
                {group.entries.map((entry) => {
                  const applying = applyingId === entry.id;
                  const applied = appliedId === entry.id;
                  const settings = [
                    [t("textToSpeech.voiceGallery.model"), entry.model],
                    [t("textToSpeech.voiceGallery.voiceId"), entry.voice],
                    ...(entry.language
                      ? [
                          [
                            t("textToSpeech.voiceGallery.language"),
                            entry.language,
                          ],
                        ]
                      : []),
                    [
                      t("textToSpeech.voiceGallery.speed"),
                      `${entry.speed.toFixed(2)}×`,
                    ],
                    ...(entry.murfStyle
                      ? [
                          [
                            t("textToSpeech.voiceGallery.style"),
                            entry.murfStyle,
                          ],
                          [
                            t("textToSpeech.voiceGallery.ratePitch"),
                            `${entry.murfRate ?? 0} / ${entry.murfPitch ?? 0}`,
                          ],
                        ]
                      : []),
                    ...(entry.voiceInstructions
                      ? [
                          [
                            t("textToSpeech.voiceGallery.instructions"),
                            entry.voiceInstructions,
                          ],
                        ]
                      : []),
                  ];

                  return (
                    <article
                      key={entry.id}
                      data-testid={`tts-voice-card-${entry.id}`}
                      className="flex min-w-0 flex-col rounded-xl border border-white/[0.07] bg-black/20 p-4"
                    >
                      <div className="flex items-start justify-between gap-3">
                        <div className="min-w-0">
                          <h4 className="truncate text-base font-semibold text-[#f5f5f5]">
                            {entry.voiceName}
                          </h4>
                          <p className="mt-0.5 text-xs text-[#b99bdc]">
                            {entry.modelFamily} · {entry.providerLabel}
                          </p>
                        </div>
                        <span className="shrink-0 rounded-full border border-violet-300/20 bg-violet-300/10 px-2 py-1 text-[10px] font-medium uppercase tracking-wider text-violet-100">
                          {entry.language || "EN"}
                        </span>
                      </div>

                      <div data-testid={`tts-voice-audio-${entry.id}`}>
                        <AudioPlayer
                          className="mt-3 rounded-lg border border-white/[0.05] bg-white/[0.025] px-3 py-2"
                          src={assetUrl(entry.asset.path)}
                        />
                      </div>

                      <p className="mt-2.5 text-xs italic leading-relaxed text-[#999]">
                        “{entry.transcript}”
                      </p>

                      <dl className="mt-3 grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-[11px] leading-relaxed">
                        {settings.map(([label, value]) => (
                          <React.Fragment key={`${label}-${value}`}>
                            <dt className="text-[#747474]">{label}</dt>
                            <dd className="break-all text-right font-mono text-[#bdbdbd]">
                              {value}
                            </dd>
                          </React.Fragment>
                        ))}
                      </dl>

                      <Button
                        type="button"
                        data-testid={`tts-voice-apply-${entry.id}`}
                        variant={applied ? "secondary" : "primary"}
                        size="sm"
                        className="mt-3 w-full"
                        disabled={savingSettings || applyingId !== null}
                        onClick={() => void onApply(entry)}
                      >
                        {applying ? (
                          <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                        ) : applied ? (
                          <CheckCircle2 className="mr-2 h-4 w-4" />
                        ) : null}
                        {applying
                          ? t("textToSpeech.voiceGallery.applying")
                          : applied
                            ? t("textToSpeech.voiceGallery.applied")
                            : t("textToSpeech.voiceGallery.apply")}
                      </Button>
                    </article>
                  );
                })}
              </div>
            </section>
          ))}
        </div>
      </AudioPlayerGroup>
    </SettingsGroup>
  );
};
