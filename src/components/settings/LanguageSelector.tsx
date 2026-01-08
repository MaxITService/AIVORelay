import React, { useState, useRef, useEffect, useMemo, useLayoutEffect } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { SettingContainer } from "../ui/SettingContainer";
import { ResetButton } from "../ui/ResetButton";
import { useSettings } from "../../hooks/useSettings";
import { useModels } from "../../hooks/useModels";
import { LANGUAGES } from "../../lib/constants/languages";

interface LanguageSelectorProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

const unsupportedModels = ["parakeet-tdt-0.6b-v2", "parakeet-tdt-0.6b-v3"];

export const LanguageSelector: React.FC<LanguageSelectorProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, resetSetting, isUpdating } = useSettings();
  const { currentModel, loadCurrentModel } = useModels();
  const [isOpen, setIsOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const dropdownRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const [dropdownPosition, setDropdownPosition] = useState<{ top: number; left: number; width: number } | null>(null);

  const selectedLanguage = getSetting("selected_language") || "auto";
  const isUnsupported = unsupportedModels.includes(currentModel);

  // Update dropdown position when open
  useLayoutEffect(() => {
    if (isOpen && buttonRef.current) {
      const rect = buttonRef.current.getBoundingClientRect();
      setDropdownPosition({
        top: rect.bottom + 4,
        left: rect.left,
        width: rect.width,
      });
    }
  }, [isOpen]);

  // Update position on scroll/resize
  useEffect(() => {
    if (!isOpen) return;

    const updatePosition = () => {
      if (buttonRef.current) {
        const rect = buttonRef.current.getBoundingClientRect();
        setDropdownPosition({
          top: rect.bottom + 4,
          left: rect.left,
          width: rect.width,
        });
      }
    };

    window.addEventListener("scroll", updatePosition, true);
    window.addEventListener("resize", updatePosition);

    return () => {
      window.removeEventListener("scroll", updatePosition, true);
      window.removeEventListener("resize", updatePosition);
    };
  }, [isOpen]);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node) &&
        buttonRef.current &&
        !buttonRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
        setSearchQuery("");
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, []);

  // Listen for model state changes to update UI reactively
  useEffect(() => {
    const modelStateUnlisten = listen("model-state-changed", () => {
      loadCurrentModel();
    });

    return () => {
      modelStateUnlisten.then((fn) => fn());
    };
  }, [loadCurrentModel]);

  useEffect(() => {
    if (isOpen && searchInputRef.current) {
      searchInputRef.current.focus();
    }
  }, [isOpen]);

  const filteredLanguages = useMemo(
    () =>
      LANGUAGES.filter((language) =>
        language.label.toLowerCase().includes(searchQuery.toLowerCase()),
      ),
    [searchQuery],
  );

  const selectedLanguageName = isUnsupported
    ? t("settings.general.language.auto")
    : LANGUAGES.find((lang) => lang.value === selectedLanguage)?.label ||
      t("settings.general.language.auto");

  const handleLanguageSelect = async (languageCode: string) => {
    await updateSetting("selected_language", languageCode);
    setIsOpen(false);
    setSearchQuery("");
  };

  const handleReset = async () => {
    await resetSetting("selected_language");
  };

  const handleToggle = () => {
    if (isUpdating("selected_language") || isUnsupported) return;
    setIsOpen(!isOpen);
  };

  const handleSearchChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setSearchQuery(event.target.value);
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter" && filteredLanguages.length > 0) {
      // Select first filtered language on Enter
      handleLanguageSelect(filteredLanguages[0].value);
    } else if (event.key === "Escape") {
      setIsOpen(false);
      setSearchQuery("");
    }
  };

  const renderDropdownPortal = () => {
    if (!isOpen || !dropdownPosition || isUpdating("selected_language") || isUnsupported) {
      return null;
    }

    return createPortal(
      <div
        ref={dropdownRef}
        className="fixed bg-[#1e1e1e] border border-mid-gray/80 rounded-lg shadow-[0_8px_32px_rgba(0,0,0,0.5)] z-[9999] max-h-60 overflow-hidden"
        style={{
          top: dropdownPosition.top,
          left: dropdownPosition.left,
          width: dropdownPosition.width,
        }}
      >
        {/* Search input */}
        <div className="p-2 border-b border-mid-gray/40">
          <input
            ref={searchInputRef}
            type="text"
            value={searchQuery}
            onChange={handleSearchChange}
            onKeyDown={handleKeyDown}
            placeholder={t("settings.general.language.searchPlaceholder")}
            className="w-full px-2 py-1 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded focus:outline-none focus:ring-1 focus:ring-logo-primary focus:border-logo-primary"
          />
        </div>

        <div className="max-h-48 overflow-y-auto">
          {filteredLanguages.length === 0 ? (
            <div className="px-2 py-2 text-sm text-mid-gray text-center">
              {t("settings.general.language.noResults")}
            </div>
          ) : (
            filteredLanguages.map((language) => (
              <button
                key={language.value}
                type="button"
                className={`w-full px-2 py-1 text-sm text-left hover:bg-logo-primary/10 transition-colors duration-150 ${
                  selectedLanguage === language.value
                    ? "bg-logo-primary/20 text-logo-primary font-semibold"
                    : ""
                }`}
                onClick={() => handleLanguageSelect(language.value)}
              >
                <div className="flex items-center justify-between">
                  <span className="truncate">{language.label}</span>
                </div>
              </button>
            ))
          )}
        </div>
      </div>,
      document.body
    );
  };

  return (
    <SettingContainer
      title={t("settings.general.language.title")}
      description={
        isUnsupported
          ? t("settings.general.language.descriptionUnsupported")
          : t("settings.general.language.description")
      }
      descriptionMode={descriptionMode}
      grouped={grouped}
      disabled={isUnsupported}
    >
      <div className="flex items-center space-x-1">
        <div className="relative">
          <button
            ref={buttonRef}
            type="button"
            className={`px-2 py-1 text-sm font-semibold bg-mid-gray/10 border border-mid-gray/80 rounded min-w-[200px] text-left flex items-center justify-between transition-all duration-150 ${
              isUpdating("selected_language") || isUnsupported
                ? "opacity-50 cursor-not-allowed"
                : "hover:bg-logo-primary/10 cursor-pointer hover:border-logo-primary"
            }`}
            onClick={handleToggle}
            disabled={isUpdating("selected_language") || isUnsupported}
          >
            <span className="truncate">{selectedLanguageName}</span>
            <svg
              className={`w-4 h-4 ml-2 transition-transform duration-200 ${
                isOpen ? "transform rotate-180" : ""
              }`}
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M19 9l-7 7-7-7"
              />
            </svg>
          </button>

          {renderDropdownPortal()}
        </div>
        <ResetButton
          onClick={handleReset}
          disabled={isUpdating("selected_language") || isUnsupported}
        />
      </div>
      {isUpdating("selected_language") && (
        <div className="absolute inset-0 bg-mid-gray/10 rounded flex items-center justify-center">
          <div className="w-4 h-4 border-2 border-logo-primary border-t-transparent rounded-full animate-spin"></div>
        </div>
      )}
    </SettingContainer>
  );
};
