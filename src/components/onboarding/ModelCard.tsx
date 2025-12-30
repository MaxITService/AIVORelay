import React from "react";
import { useTranslation } from "react-i18next";
import { Download } from "lucide-react";
import type { ModelInfo } from "@/bindings";
import { formatModelSize } from "../../lib/utils/format";
import {
  getTranslatedModelName,
  getTranslatedModelDescription,
} from "../../lib/utils/modelTranslation";
import Badge from "../ui/Badge";

interface ModelCardProps {
  model: ModelInfo;
  variant?: "default" | "featured";
  disabled?: boolean;
  className?: string;
  onSelect: (modelId: string) => void;
}

const ModelCard: React.FC<ModelCardProps> = ({
  model,
  variant = "default",
  disabled = false,
  className = "",
  onSelect,
}) => {
  const { t } = useTranslation();
  const isFeatured = variant === "featured";

  // Get translated model name and description
  const displayName = getTranslatedModelName(model, t);
  const displayDescription = getTranslatedModelDescription(model, t);

  const baseButtonClasses =
    "flex justify-between items-center rounded-xl p-4 px-5 text-left transition-all duration-200 disabled:opacity-40 disabled:cursor-not-allowed focus:outline-none focus:ring-2 focus:ring-[#ff4d8d]/30 active:scale-[0.98] cursor-pointer group";

  const variantClasses = isFeatured
    ? "glass-panel border border-[#ff4d8d]/30 hover:border-[#ff4d8d]/50 hover:shadow-[0_8px_32px_rgba(255,77,141,0.2)] hover:-translate-y-0.5 disabled:hover:border-[#ff4d8d]/30 disabled:hover:shadow-none disabled:hover:translate-y-0"
    : "glass-panel-subtle hover:border-[#3d3d3d] hover:shadow-[0_8px_24px_rgba(0,0,0,0.5)] hover:-translate-y-0.5 disabled:hover:border-[#282828] disabled:hover:shadow-none disabled:hover:translate-y-0";

  return (
    <button
      onClick={() => onSelect(model.id)}
      disabled={disabled}
      className={[baseButtonClasses, variantClasses, className]
        .filter(Boolean)
        .join(" ")}
      type="button"
    >
      <div className="flex flex-col">
        <div className="flex items-center gap-3">
          <h3 className="text-lg font-semibold text-[#f5f5f5] group-hover:text-[#ff4d8d] transition-colors">
            {displayName}
          </h3>
          <DownloadSize sizeMb={Number(model.size_mb)} />
          {isFeatured && (
            <Badge variant="primary">{t("onboarding.recommended")}</Badge>
          )}
        </div>
        <p className="text-[#a0a0a0] text-sm leading-relaxed mt-1">
          {displayDescription}
        </p>
      </div>

      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <p className="text-xs text-[#6b6b6b] w-16 text-right">
            {t("onboarding.modelCard.accuracy")}
          </p>
          <div className="w-20 h-1.5 bg-[#2b2b2b] rounded-full overflow-hidden">
            <div
              className="h-full bg-gradient-to-r from-[#ff4d8d] to-[#9b5de5] rounded-full transition-all duration-300"
              style={{ width: `${model.accuracy_score * 100}%` }}
            />
          </div>
        </div>
        <div className="flex items-center gap-2">
          <p className="text-xs text-[#6b6b6b] w-16 text-right">
            {t("onboarding.modelCard.speed")}
          </p>
          <div className="w-20 h-1.5 bg-[#2b2b2b] rounded-full overflow-hidden">
            <div
              className="h-full bg-gradient-to-r from-[#ff4d8d] to-[#9b5de5] rounded-full transition-all duration-300"
              style={{ width: `${model.speed_score * 100}%` }}
            />
          </div>
        </div>
      </div>
    </button>
  );
};

const DownloadSize = ({ sizeMb }: { sizeMb: number }) => {
  const { t } = useTranslation();

  return (
    <div className="flex items-center gap-1.5 text-xs text-[#6b6b6b] tabular-nums">
      <Download
        aria-hidden="true"
        className="h-3.5 w-3.5 text-[#4a4a4a]"
        strokeWidth={1.75}
      />
      <span className="sr-only">{t("modelSelector.downloadSize")}</span>
      <span className="font-medium text-[#a0a0a0]">
        {formatModelSize(sizeMb)}
      </span>
    </div>
  );
};

export default ModelCard;
