import React, { useState, useEffect } from "react";
import { getVersion } from "@tauri-apps/api/app";

import ModelSelector from "../model-selector";
import UpdateChecker from "../update-checker";
import VramMeter from "./VramMeter";

const Footer: React.FC = () => {
  const [version, setVersion] = useState("");
  const [vramRefreshNonce, setVramRefreshNonce] = useState(0);

  useEffect(() => {
    const fetchVersion = async () => {
      try {
        const appVersion = await getVersion();
        setVersion(appVersion);
      } catch (error) {
        console.error("Failed to get app version:", error);
        setVersion("0.1.2");
      }
    };

    fetchVersion();
  }, []);

  return (
    <div className="w-full bg-[#0f0f0f] border-t border-[#282828] pt-3">
      <div className="flex justify-between items-center text-xs px-4 pb-3 text-[#b8b8b8]">
        <div className="flex items-center gap-4">
          <ModelSelector
            onInteraction={() => setVramRefreshNonce((prev) => prev + 1)}
          />
          <VramMeter refreshNonce={vramRefreshNonce} />
        </div>

        {/* Update Status */}
        <div className="flex items-center gap-2">
          <UpdateChecker />
          <span className="text-[#333333]">•</span>
          {/* eslint-disable-next-line i18next/no-literal-string */}
          <span className="font-medium">v{version}</span>
        </div>
      </div>
    </div>
  );
};

export default Footer;
