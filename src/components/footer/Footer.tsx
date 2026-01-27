import React, { useState, useEffect } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { open } from "@tauri-apps/plugin-opener";

import ModelSelector from "../model-selector";

const Footer: React.FC = () => {
  const [version, setVersion] = useState("");

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

  const openReleasesPage = () => {
    void open(
      "https://github.com/MaxITService/AIVORelay/releases?q=avx2&expanded=true",
    );
  };

  return (
    <div className="w-full bg-[#0f0f0f] border-t border-[#282828] pt-3">
      <div className="flex justify-between items-center text-xs px-4 pb-3 text-[#b8b8b8]">
        <div className="flex items-center gap-4">
          <ModelSelector />
        </div>

        {/* Version & Manual Update Link (AVX2 build) */}
        <div className="flex items-center gap-2">
          <button
            onClick={openReleasesPage}
            className="text-text/60 hover:text-text/80 transition-colors"
          >
            {/* eslint-disable-next-line i18next/no-literal-string */}
            Update manually
          </button>
          <span className="text-[#333333]">•</span>
          {/* eslint-disable-next-line i18next/no-literal-string */}
          <span className="font-medium">v{version} (AVX2)</span>
        </div>
      </div>
    </div>
  );
};

export default Footer;
