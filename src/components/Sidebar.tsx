import React from "react";
import { useTranslation } from "react-i18next";
import { Cog, FlaskConical, Globe, History, Info, Sparkles, Wand2, Terminal } from "lucide-react";
import { type } from "@tauri-apps/plugin-os";
import HandyTextLogo from "./icons/HandyTextLogo";
import HandyHand from "./icons/HandyHand";
import { useSettings } from "../hooks/useSettings";
import {
  GeneralSettings,
  AdvancedSettings,
  HistorySettings,
  DebugSettings,
  AboutSettings,
  PostProcessingSettings,
  BrowserConnectorSettings,
  AiReplaceSelectionSettings,
  VoiceCommandSettings,
} from "./settings";

export type SidebarSection = keyof typeof SECTIONS_CONFIG;

interface IconProps {
  width?: number | string;
  height?: number | string;
  size?: number | string;
  className?: string;
  [key: string]: any;
}

interface SectionConfig {
  labelKey: string;
  icon: React.ComponentType<IconProps>;
  component: React.ComponentType;
  enabled: (settings: any) => boolean;
}

const isWindows = type() === "windows";

export const SECTIONS_CONFIG = {
  general: {
    labelKey: "sidebar.general",
    icon: HandyHand,
    component: GeneralSettings,
    enabled: () => true,
  },
  advanced: {
    labelKey: "sidebar.advanced",
    icon: Cog,
    component: AdvancedSettings,
    enabled: () => true,
  },
  postprocessing: {
    labelKey: "sidebar.postProcessing",
    icon: Sparkles,
    component: PostProcessingSettings,
    enabled: (_) => true,
  },
  aiReplace: {
    labelKey: "sidebar.aiReplace",
    icon: Wand2,
    component: AiReplaceSelectionSettings,
    enabled: () => isWindows,
  },
  voiceCommands: {
    labelKey: "sidebar.voiceCommands",
    icon: Terminal,
    component: VoiceCommandSettings,
    enabled: () => isWindows,
  },
  browserConnector: {
    labelKey: "sidebar.browserConnector",
    icon: Globe,
    component: BrowserConnectorSettings,
    enabled: () => true,
  },
  history: {
    labelKey: "sidebar.history",
    icon: History,
    component: HistorySettings,
    enabled: (_) => true,
  },
  debug: {
    labelKey: "sidebar.debug",
    icon: FlaskConical,
    component: DebugSettings,
    enabled: (_) => true,
  },
  about: {
    labelKey: "sidebar.about",
    icon: Info,
    component: AboutSettings,
    enabled: () => true,
  },
} as const satisfies Record<string, SectionConfig>;

interface SidebarProps {
  activeSection: SidebarSection;
  onSectionChange: (section: SidebarSection) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeSection,
  onSectionChange,
}) => {
  const { t } = useTranslation();
  const { settings } = useSettings();

  const availableSections = Object.entries(SECTIONS_CONFIG)
    .filter(([_, config]) => config.enabled(settings))
    .map(([id, config]) => ({ id: id as SidebarSection, ...config }));

  return (
    <div className="adobe-sidebar flex flex-col w-52 h-full items-center px-3 py-4">
      {/* Logo Section with glow effect */}
      <div className="w-full p-3 mb-2">
        <HandyTextLogo className="w-full h-auto drop-shadow-[0_0_8px_rgba(255,107,157,0.3)]" />
      </div>
      
      {/* Gradient Divider */}
      <div className="section-divider w-full mb-4" />
      
      {/* Navigation Items */}
      <div className="flex flex-col w-full gap-1">
        {availableSections.map((section) => {
          const Icon = section.icon;
          const isActive = activeSection === section.id;

          return (
            <div
              key={section.id}
              className={`adobe-sidebar-item flex gap-3 items-center w-full ${
                isActive ? "active" : ""
              }`}
              onClick={() => onSectionChange(section.id)}
            >
              {/* Icon with gradient on active */}
              <div className={`shrink-0 transition-all duration-200 ${
                isActive 
                  ? "text-[#ff4d8d] drop-shadow-[0_0_6px_rgba(255,77,141,0.5)]" 
                  : "text-[#b8b8b8] group-hover:text-[#f5f5f5]"
              }`}>
                <Icon width={20} height={20} />
              </div>
              
              {/* Label */}
              <p
                className={`text-sm font-medium truncate transition-colors duration-200 ${
                  isActive 
                    ? "text-[#f5f5f5]" 
                    : "text-[#b8b8b8]"
                }`}
                title={t(section.labelKey)}
              >
                {t(section.labelKey)}
              </p>
            </div>
          );
        })}
      </div>
      
      {/* Bottom spacer with subtle gradient */}
      <div className="flex-1" />
      <div className="section-divider w-full mt-4" />
      <div className="w-full py-3 px-2 text-center">
        <span className="text-xs text-[#707070]">AivoRelay</span>
      </div>
    </div>
  );
};
