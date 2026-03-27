import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface AppRuntimeInfo {
  executableName: string | null;
  executableVariant: string;
  selfUpdateSupported: boolean;
}

let runtimeInfoPromise: Promise<AppRuntimeInfo> | null = null;

async function fetchAppRuntimeInfo(): Promise<AppRuntimeInfo> {
  return invoke<AppRuntimeInfo>("get_app_runtime_info");
}

export function useAppRuntimeInfo() {
  const [runtimeInfo, setRuntimeInfo] = useState<AppRuntimeInfo | null>(null);

  useEffect(() => {
    let cancelled = false;

    if (!runtimeInfoPromise) {
      runtimeInfoPromise = fetchAppRuntimeInfo();
    }

    runtimeInfoPromise
      .then((info) => {
        if (!cancelled) {
          setRuntimeInfo(info);
        }
      })
      .catch((error) => {
        console.error("Failed to load app runtime info:", error);
        if (!cancelled) {
          setRuntimeInfo({
            executableName: null,
            executableVariant: "standard",
            selfUpdateSupported: true,
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return runtimeInfo;
}
