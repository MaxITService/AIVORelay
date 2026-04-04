import React from "react";
import { ShieldAlert, Download, FolderOpen, ExternalLink, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Button } from "../../ui/Button";

type ExternalModelDownloadModalProps = {
  isOpen: boolean;
  modelName: string;
  sourceLabel: string;
  sourceUrl: string;
  privacyUrl: string;
  termsUrl: string;
  destinationPath: string;
  files: string[];
  onAccept: () => void | Promise<void>;
  onOpenFolder: () => void | Promise<void>;
  onClose: () => void;
};

export const ExternalModelDownloadModal: React.FC<ExternalModelDownloadModalProps> = ({
  isOpen,
  modelName,
  sourceLabel,
  sourceUrl,
  privacyUrl,
  termsUrl,
  destinationPath,
  files,
  onAccept,
  onOpenFolder,
  onClose,
}) => {
  const { t } = useTranslation();

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" onClick={onClose}>
      <div className="absolute inset-0 bg-black/70 backdrop-blur-sm" />

      <div
        className="relative z-10 w-full max-w-3xl mx-4 rounded-2xl border border-[#4a3a1d] bg-[#131313]/95 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <button
          onClick={onClose}
          className="absolute top-3 right-3 p-1 rounded-md text-text/60 hover:text-text hover:bg-mid-gray/20 transition-colors"
          aria-label={t("common.close", "Close")}
        >
          <X className="w-5 h-5" />
        </button>

        <div className="p-6 md:p-7 space-y-6">
          <div className="flex items-start gap-4">
            <div className="p-3 rounded-full bg-[#3c2f14]/70 text-yellow-300">
              <ShieldAlert className="w-6 h-6" />
            </div>
            <div className="space-y-2">
              <h2 className="text-lg font-semibold text-[#f5f5f5]">
                {t("modelSelector.externalSourceDialog.title", { modelName })}
              </h2>
              <p className="text-sm text-text/80 leading-relaxed">
                {t("modelSelector.externalSourceDialog.body", { sourceLabel })}
              </p>
            </div>
          </div>

          <div className="rounded-xl border border-[#3a3a3a] bg-[#1a1a1a]/70 p-4 space-y-3">
            <p className="text-xs uppercase tracking-[0.18em] text-[#8a8a8a]">
              {t("modelSelector.externalSourceDialog.networkTitle")}
            </p>
            <p className="text-sm text-text/80 leading-relaxed">
              {t("modelSelector.externalSourceDialog.networkBody")}
            </p>
            <div className="flex flex-wrap gap-2">
              <Button variant="secondary" size="sm" onClick={() => openUrl(sourceUrl)}>
                <ExternalLink className="w-3.5 h-3.5 mr-1.5" />
                {t("modelSelector.externalSourceDialog.openSource")}
              </Button>
              <Button variant="ghost" size="sm" onClick={() => openUrl(privacyUrl)}>
                <ExternalLink className="w-3.5 h-3.5 mr-1.5" />
                {t("modelSelector.externalSourceDialog.openPrivacy")}
              </Button>
              <Button variant="ghost" size="sm" onClick={() => openUrl(termsUrl)}>
                <ExternalLink className="w-3.5 h-3.5 mr-1.5" />
                {t("modelSelector.externalSourceDialog.openTerms")}
              </Button>
            </div>
          </div>

          <div className="grid gap-4 md:grid-cols-[1.25fr_0.95fr]">
            <div className="rounded-xl border border-[#3a3a3a] bg-[#171717]/70 p-4 space-y-3">
              <p className="text-xs uppercase tracking-[0.18em] text-[#8a8a8a]">
                {t("modelSelector.externalSourceDialog.manualTitle")}
              </p>
              <p className="text-sm text-text/80 leading-relaxed">
                {t("modelSelector.externalSourceDialog.manualBody")}
              </p>
              <div className="rounded-lg bg-black/25 border border-[#2f2f2f] p-3">
                <p className="text-xs text-[#8a8a8a] mb-2">
                  {t("modelSelector.externalSourceDialog.destinationLabel")}
                </p>
                <code className="text-xs break-all text-[#f5f5f5]">{destinationPath}</code>
              </div>
            </div>

            <div className="rounded-xl border border-[#3a3a3a] bg-[#171717]/70 p-4 space-y-3">
              <p className="text-xs uppercase tracking-[0.18em] text-[#8a8a8a]">
                {t("modelSelector.externalSourceDialog.filesTitle")}
              </p>
              <div className="max-h-56 overflow-y-auto rounded-lg bg-black/25 border border-[#2f2f2f] p-3">
                <ul className="space-y-2 text-xs text-[#f5f5f5]">
                  {files.map((file) => (
                    <li key={file}>
                      <code className="break-all">{file}</code>
                    </li>
                  ))}
                </ul>
              </div>
            </div>
          </div>

          <div className="flex flex-col-reverse gap-3 md:flex-row md:items-center md:justify-between">
            <div className="flex flex-wrap gap-2">
              <Button variant="ghost" size="sm" onClick={onClose}>
                {t("common.cancel")}
              </Button>
              <Button variant="secondary" size="sm" onClick={onOpenFolder}>
                <FolderOpen className="w-3.5 h-3.5 mr-1.5" />
                {t("modelSelector.externalSourceDialog.openDestination")}
              </Button>
            </div>
            <Button
              variant="primary"
              size="md"
              onClick={async () => {
                await onAccept();
                onClose();
              }}
            >
              <Download className="w-4 h-4 mr-2" />
              {t("modelSelector.externalSourceDialog.acceptAndDownload")}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
};
