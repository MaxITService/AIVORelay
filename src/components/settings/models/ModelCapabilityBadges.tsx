import React from "react";
import { useTranslation } from "react-i18next";
import type { SttModelSelection } from "@/bindings";
import { sttModelCapabilities } from "@/lib/sttModelSelection";

export const ModelCapabilityBadges: React.FC<{
  selection: SttModelSelection;
}> = ({ selection }) => {
  const { t } = useTranslation();
  // A custom endpoint's capabilities depend on the server behind it.
  if (selection.provider === "remote_openai_compatible" &&
      selection.provider_preset === "custom") {
    return <p className="mt-2 text-[11px] text-[#a0a0a0]">
      {t("modelSelector.cardCapabilities.custom", "Capabilities depend on your endpoint and model.")}
    </p>;
  }

  const capabilities = sttModelCapabilities(selection);
  const model = (selection.model_id || "").toLowerCase();
  const files = capabilities.workflows.includes("file") &&
    (!/realtime|transcribe-live|live-transcribe/.test(model) ||
      selection.provider_preset === "elevenlabs");
  const live = capabilities.workflows.includes("live") ||
    (selection.provider_preset === "openai" &&
      ["gpt-live-transcribe", "gpt-realtime-whisper"].includes(model));
  const subtitles = files && (capabilities.timestamps.includes("file") ||
    (selection.provider_preset === "groq" && model.includes("whisper")));
  const badges = [
    t("modelSelector.cardCapabilities.dictation", "Dictation"),
    ...(live ? [t("modelSelector.cardCapabilities.live", "Live dictation")] : []),
    ...(files ? [t("modelSelector.cardCapabilities.files", "Files")] : []),
    ...(subtitles ? [t("modelSelector.cardCapabilities.subtitles", "Subtitles (SRT/VTT)")] : []),
    ...(capabilities.diarization.includes("file") ? [t("modelSelector.cardCapabilities.speakersFile", "Speakers in files")] : []),
    ...(capabilities.diarization.includes("live") ? [t("modelSelector.cardCapabilities.speakersLive", "Speakers in Live Monitor")] : []),
  ];

  return <div className="mt-2 flex flex-wrap gap-1.5">
    {badges.map((badge) => <span key={badge}
      className="rounded-full border border-[#3d3d3d] bg-[#1b1b1b] px-2 py-0.5 text-[10px] text-[#b8b8b8]">
      {badge}
    </span>)}
  </div>;
};
