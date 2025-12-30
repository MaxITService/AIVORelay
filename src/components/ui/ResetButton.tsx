import React from "react";
import ResetIcon from "../icons/ResetIcon";

interface ResetButtonProps {
  onClick: () => void;
  disabled?: boolean;
  className?: string;
  ariaLabel?: string;
  children?: React.ReactNode;
}

export const ResetButton: React.FC<ResetButtonProps> = React.memo(
  ({ onClick, disabled = false, className = "", ariaLabel, children }) => (
    <button
      type="button"
      aria-label={ariaLabel}
      className={`p-1.5 rounded-md border border-transparent transition-all duration-200 ${
        disabled
          ? "opacity-40 cursor-not-allowed text-[#4a4a4a]"
          : "hover:bg-[#ff4d8d]/20 active:bg-[#ff4d8d]/30 active:translate-y-[1px] hover:cursor-pointer hover:border-[#ff4d8d]/50 text-[#b8b8b8] hover:text-[#ff4d8d]"
      } ${className}`}
      onClick={onClick}
      disabled={disabled}
    >
      {children ?? <ResetIcon />}
    </button>
  ),
);
