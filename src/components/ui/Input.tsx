import React from "react";

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  variant?: "default" | "compact";
}

export const Input: React.FC<InputProps> = ({
  className = "",
  variant = "default",
  disabled,
  ...props
}) => {
  const baseClasses =
    "text-sm font-medium bg-[#1e1e1e]/80 border border-[#3c3c3c] rounded-md text-[#e8e8e8] placeholder-[#6b6b6b] transition-all duration-200";

  const interactiveClasses = disabled
    ? "opacity-50 cursor-not-allowed bg-[#1a1a1a]/60 border-[#282828]"
    : "hover:border-[#3d3d3d] hover:bg-[#1a1a1a]/80 focus:outline-none focus:border-[#ff4d8d] focus:shadow-[0_0_0_2px_rgba(255,77,141,0.2)]";

  const variantClasses = {
    default: "px-3 py-2",
    compact: "px-2 py-1.5",
  } as const;

  return (
    <input
      className={`${baseClasses} ${variantClasses[variant]} ${interactiveClasses} ${className}`}
      disabled={disabled}
      {...props}
    />
  );
};
