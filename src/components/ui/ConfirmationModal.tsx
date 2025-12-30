import React from "react";
import { AlertTriangle, X } from "lucide-react";
import { Button } from "./Button";

interface ConfirmationModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title: string;
  message: string;
  confirmText: string;
  cancelText?: string;
  variant?: "warning" | "danger";
}

export const ConfirmationModal: React.FC<ConfirmationModalProps> = ({
  isOpen,
  onClose,
  onConfirm,
  title,
  message,
  confirmText,
  cancelText = "Cancel",
  variant = "warning",
}) => {
  if (!isOpen) return null;

  const borderColor = variant === "danger" ? "border-red-500/50" : "border-yellow-500/50";
  const bgGradient = variant === "danger" 
    ? "from-red-500/10 to-red-600/5" 
    : "from-yellow-500/10 to-orange-500/5";
  const iconColor = variant === "danger" ? "text-red-400" : "text-yellow-400";
  const titleColor = variant === "danger" ? "text-red-300" : "text-yellow-300";
  const confirmVariant = variant === "danger" ? "danger" : "primary";

  return (
    <div 
      className="fixed inset-0 z-50 flex items-center justify-center"
      onClick={onClose}
    >
      {/* Backdrop with blur */}
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" />
      
      {/* Modal */}
      <div 
        className={`
          relative z-10 w-full max-w-md mx-4
          bg-gradient-to-br ${bgGradient}
          border-2 ${borderColor}
          rounded-xl shadow-2xl
          backdrop-blur-md
          animate-in fade-in zoom-in-95 duration-200
        `}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Close button */}
        <button
          onClick={onClose}
          className="absolute top-3 right-3 p-1 rounded-md text-text/60 hover:text-text hover:bg-mid-gray/20 transition-colors"
        >
          <X className="w-5 h-5" />
        </button>

        {/* Content */}
        <div className="p-6">
          {/* Icon and Title */}
          <div className="flex items-center gap-3 mb-4">
            <div className={`p-2 rounded-full bg-black/30 ${iconColor}`}>
              <AlertTriangle className="w-6 h-6" />
            </div>
            <h2 className={`text-lg font-semibold ${titleColor}`}>
              {title}
            </h2>
          </div>

          {/* Message */}
          <p className="text-text/80 text-sm leading-relaxed mb-6">
            {message}
          </p>

          {/* Actions */}
          <div className="flex gap-3 justify-end">
            <Button
              variant="ghost"
              onClick={onClose}
            >
              {cancelText}
            </Button>
            <Button
              variant={confirmVariant}
              onClick={() => {
                onConfirm();
                onClose();
              }}
            >
              {confirmText}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
};
