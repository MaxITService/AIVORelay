import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink } from "lucide-react";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { Button } from "../../ui/Button";
import { AppDataDirectory } from "../AppDataDirectory";
import { AppLanguageSelector } from "../AppLanguageSelector";
import oneClickPromptsScreenshot from "../../../assets/OneClickPrompts-screenshot.png";

export const AboutSettings: React.FC = () => {
  const { t } = useTranslation();
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

  const handleDonateClick = async () => {
    try {
      await openUrl("https://handy.computer/donate");
    } catch (error) {
      console.error("Failed to open donate link:", error);
    }
  };

  const handleForkDonateClick = async () => {
    try {
      await openUrl("https://buymeacoffee.com/netstaff");
    } catch (error) {
      console.error("Failed to open fork donate link:", error);
    }
  };

  const handleContactAuthorClick = async () => {
    try {
      await openUrl("mailto:forpphotos@gmail.com");
    } catch (error) {
      console.error("Failed to open email client:", error);
    }
  };

  const handleReportIssuesClick = async () => {
    try {
      await openUrl("https://github.com/MaxITService/AIVORelay/issues");
    } catch (error) {
      console.error("Failed to open issues page:", error);
    }
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.about.title")}>
        <AppLanguageSelector descriptionMode="tooltip" grouped={true} />
        <SettingContainer
          title={t("settings.about.version.title")}
          description={t("settings.about.version.description")}
          grouped={true}
        >
          {/* eslint-disable-next-line i18next/no-literal-string */}
          <span className="text-sm font-mono">v{version}</span>
        </SettingContainer>

        <AppDataDirectory descriptionMode="tooltip" grouped={true} />

        <SettingContainer
          title={t("settings.about.sourceCode.title")}
          description={t("settings.about.sourceCode.description")}
          grouped={true}
        >
          <Button
            variant="secondary"
            size="md"
            onClick={() => openUrl("https://github.com/MaxITService/AIVORelay")}
          >
            {t("settings.about.sourceCode.button")}
          </Button>
        </SettingContainer>

        <SettingContainer
          title={t("settings.about.supportDevelopment.title")}
          description={t("settings.about.supportDevelopment.description")}
          grouped={true}
        >
          <Button variant="primary" size="md" onClick={handleDonateClick}>
            {t("settings.about.supportDevelopment.button")}
          </Button>
        </SettingContainer>

        <SettingContainer
          title={t("settings.about.supportFork.title")}
          description={t("settings.about.supportFork.description")}
          grouped={true}
        >
          <Button variant="primary" size="md" onClick={handleForkDonateClick}>
            {t("settings.about.supportFork.button")}
          </Button>
        </SettingContainer>

        <SettingContainer
          title={t("settings.about.contactAuthor.title")}
          description={t("settings.about.contactAuthor.description")}
          grouped={true}
        >
          <Button variant="secondary" size="md" onClick={handleContactAuthorClick}>
            {t("settings.about.contactAuthor.button")}
          </Button>
        </SettingContainer>

        <SettingContainer
          title={t("settings.about.reportIssues.title")}
          description={t("settings.about.reportIssues.description")}
          grouped={true}
        >
          <Button variant="secondary" size="md" onClick={handleReportIssuesClick}>
            {t("settings.about.reportIssues.button")}
          </Button>
        </SettingContainer>
      </SettingsGroup>

      <SettingsGroup title={t("settings.about.forkInfo.title")}>
        <SettingContainer
          title={t("settings.about.forkInfo.title")}
          description={t("settings.about.forkInfo.description")}
          grouped={true}
        >
          <Button
            variant="secondary"
            size="md"
            onClick={() => openUrl("https://github.com/cjpais/Handy")}
          >
            {t("settings.about.forkInfo.button")}
          </Button>
        </SettingContainer>

        <SettingContainer
          title={t("settings.about.license.title")}
          description={t("settings.about.license.description")}
          grouped={true}
        >
          <Button
            variant="secondary"
            size="md"
            onClick={() => openUrl("https://github.com/cjpais/Handy/blob/main/LICENSE")}
          >
            {t("settings.about.license.viewUpstream")}
          </Button>
        </SettingContainer>
      </SettingsGroup>

      <SettingsGroup title={t("settings.about.acknowledgments.title")}>
        <SettingContainer
          title={t("settings.about.acknowledgments.whisper.title")}
          description={t("settings.about.acknowledgments.whisper.description")}
          grouped={true}
          layout="stacked"
        >
          <div className="text-sm text-mid-gray">
            {t("settings.about.acknowledgments.whisper.details")}
          </div>
        </SettingContainer>
        <SettingContainer
          title={t("settings.about.acknowledgments.vulkan.title")}
          description={t("settings.about.acknowledgments.vulkan.description")}
          grouped={true}
          layout="stacked"
        >
          <div className="text-sm text-mid-gray">
            {t("settings.about.acknowledgments.vulkan.details")}
          </div>
        </SettingContainer>
      </SettingsGroup>

      <SettingsGroup title={t("settings.about.moreProjects.title")}>
        {/* OneClickPrompts - featured with screenshot */}
        <div className="p-4 space-y-3">
          <button
            onClick={() => openUrl("https://github.com/MaxITService/OneClickPrompts")}
            className="group cursor-pointer text-left w-full"
          >
            <div className="flex items-center gap-2 mb-2">
              <span className="text-sm font-semibold text-text group-hover:text-[#ff4d8d] transition-colors">
                {t("settings.about.moreProjects.oneClickPrompts.title")}
              </span>
              <ExternalLink className="w-3.5 h-3.5 text-mid-gray group-hover:text-[#ff4d8d] transition-colors" />
            </div>
            <p className="text-xs text-mid-gray mb-3">
              {t("settings.about.moreProjects.oneClickPrompts.description")}
            </p>
            <div className="inline-block rounded-lg border border-[#3c3c3c] overflow-hidden shadow-lg">
              <img
                src={oneClickPromptsScreenshot}
                alt="OneClickPrompts"
                className="max-w-full h-auto block"
              />
            </div>
          </button>
        </div>

        {/* Other projects as links */}
        <div className="divide-y divide-white/[0.05]">
          <button
            onClick={() => openUrl("https://github.com/MaxITService/Console2Ai")}
            className="group w-full flex items-center justify-between px-4 py-3 cursor-pointer hover:bg-white/[0.02] transition-colors"
          >
            <div className="text-left">
              <span className="text-sm font-medium text-text group-hover:text-[#ff4d8d] transition-colors">
                {t("settings.about.moreProjects.console2Ai.title")}
              </span>
              <p className="text-xs text-mid-gray">
                {t("settings.about.moreProjects.console2Ai.description")}
              </p>
            </div>
            <ExternalLink className="w-4 h-4 text-mid-gray group-hover:text-[#ff4d8d] transition-colors flex-shrink-0" />
          </button>

          <button
            onClick={() => openUrl("https://github.com/MaxITService/Ping-Plotter-PS51")}
            className="group w-full flex items-center justify-between px-4 py-3 cursor-pointer hover:bg-white/[0.02] transition-colors"
          >
            <div className="text-left">
              <span className="text-sm font-medium text-text group-hover:text-[#ff4d8d] transition-colors">
                {t("settings.about.moreProjects.pingPlotter.title")}
              </span>
              <p className="text-xs text-mid-gray">
                {t("settings.about.moreProjects.pingPlotter.description")}
              </p>
            </div>
            <ExternalLink className="w-4 h-4 text-mid-gray group-hover:text-[#ff4d8d] transition-colors flex-shrink-0" />
          </button>

          <button
            onClick={() => openUrl("https://medium.com/@maxim.fomins/ai-for-complete-beginners-guide-llms-f19c4b8a8a79")}
            className="group w-full flex items-center justify-between px-4 py-3 cursor-pointer hover:bg-white/[0.02] transition-colors"
          >
            <div className="text-left">
              <span className="text-sm font-medium text-text group-hover:text-[#ff4d8d] transition-colors">
                {t("settings.about.moreProjects.aiForBeginners.title")}
              </span>
              <p className="text-xs text-mid-gray">
                {t("settings.about.moreProjects.aiForBeginners.description")}
              </p>
            </div>
            <ExternalLink className="w-4 h-4 text-mid-gray group-hover:text-[#ff4d8d] transition-colors flex-shrink-0" />
          </button>
        </div>
      </SettingsGroup>
    </div>
  );
};
