import React, { useState } from "react";
import { ChevronDown, ChevronUp, Info } from "lucide-react";
import { useTranslation } from "react-i18next";

interface TellMeMoreProps {
  title?: string;
  children: React.ReactNode;
}

export const TellMeMore: React.FC<TellMeMoreProps> = ({ title, children }) => {
  const [isOpen, setIsOpen] = useState(false);
  const { t } = useTranslation();

  return (
    <div className="mb-6 rounded-lg border border-mid-gray/30 bg-mid-gray/5 overflow-hidden transition-all duration-300">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="flex w-full items-center justify-between p-3 text-sm font-medium text-text hover:bg-mid-gray/10 transition-colors cursor-pointer"
      >
        <div className="flex items-center gap-2 text-accent">
          <Info className="h-4 w-4" />
          <span>{title || t("common.tellMeMore", "Tell me more: How to use")}</span>
        </div>
        {isOpen ? (
          <ChevronUp className="h-4 w-4 text-text/60" />
        ) : (
          <ChevronDown className="h-4 w-4 text-text/60" />
        )}
      </button>
      
      {isOpen && (
        <div className="px-4 pb-4 pt-1 text-sm text-text/90 leading-relaxed border-t border-mid-gray/20 animate-in slide-in-from-top-2 duration-200">
          {children}
        </div>
      )}
    </div>
  );
};
