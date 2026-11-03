import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LANGUAGE_DETECTOR_ID } from "@/components/settings/languageDetectorDownload";

export type DetectorProgress = { model_id: string; downloaded: number; total: number; percentage: number };
type DetectorStatus = { downloaded: boolean; progress: DetectorProgress | null };

export function useLanguageDetectorDownload() {
  const [status, setStatus] = useState<DetectorStatus | null>(null);
  const [error, setError] = useState("");
  const [starting, setStarting] = useState(false);

  useEffect(() => {
    let active = true;
    let revision = 0;
    const stops: (() => void)[] = [];
    const refresh = async () => {
      const requested = ++revision;
      try {
        const next = await invoke<DetectorStatus>("get_language_detector_download_status");
        if (active && requested === revision) setStatus(next);
      } catch (reason) { if (active && requested === revision) setError(String(reason)); }
    };
    const register = async () => {
      const registrations = [
        listen<DetectorProgress>("model-download-progress", ({ payload }) => {
          if (!active || payload.model_id !== LANGUAGE_DETECTOR_ID) return;
          revision++;
          setError("");
          setStatus((current) => ({ downloaded: current?.downloaded ?? false, progress: payload }));
        }),
        listen<string>("model-download-complete", ({ payload }) => {
          if (!active || payload !== LANGUAGE_DETECTOR_ID) return;
          revision++;
          setError("");
          setStatus({ downloaded: true, progress: null });
        }),
        listen<string>("model-download-cancelled", ({ payload }) => {
          if (!active || payload !== LANGUAGE_DETECTOR_ID) return;
          revision++;
          setStatus({ downloaded: false, progress: null });
        }),
        listen<{ model_id: string; error: string }>("model-download-failed", ({ payload }) => {
          if (!active || payload.model_id !== LANGUAGE_DETECTOR_ID) return;
          revision++;
          setStatus({ downloaded: false, progress: null });
          setError(payload.error);
        }),
        listen("models-updated", () => { void refresh(); }),
      ];
      await Promise.all(registrations.map(async (registration) => {
        try {
          const stop = await registration;
          if (active) stops.push(stop); else stop();
        } catch (reason) { if (active) setError(String(reason)); }
      }));
      if (active) await refresh();
    };
    void register();
    return () => { active = false; stops.forEach((stop) => stop()); };
  }, []);

  const download = async () => {
    setStarting(true);
    setError("");
    try { await invoke("download_language_detector"); }
    catch (reason) { setError(String(reason)); }
    finally { setStarting(false); }
  };
  const cancel = async () => {
    try { await invoke("cancel_language_detector_download"); }
    catch (reason) { setError(String(reason)); }
  };
  return { status, error, starting, download, cancel };
}
