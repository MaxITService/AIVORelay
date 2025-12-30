import React from "react";

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "danger" | "ghost";
  size?: "sm" | "md" | "lg";
}

export const Button: React.FC<ButtonProps> = ({
  children,
  className = "",
  variant = "primary",
  size = "md",
  ...props
}) => {
  const baseClasses =
    "font-medium rounded-md focus:outline-none transition-all duration-200 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer";

  const variantClasses = {
    primary:
      "text-white bg-gradient-to-r from-[#ff4d8d] to-[#9b5de5] border-none hover:shadow-[0_4px_16px_rgba(255,77,141,0.4)] hover:-translate-y-0.5 active:translate-y-0",
    secondary:
      "bg-[#1a1a1a]/70 border border-[#333333] text-[#f5f5f5] backdrop-blur-sm hover:bg-[#222222]/80 hover:border-[#3d3d3d] hover:shadow-[0_4px_12px_rgba(0,0,0,0.4)]",
    danger:
      "text-white bg-gradient-to-r from-[#ff4757] to-[#ff6b7a] border-none hover:shadow-[0_4px_16px_rgba(255,71,87,0.4)] hover:-translate-y-0.5",
    ghost:
      "text-[#b8b8b8] border border-transparent bg-transparent hover:bg-[#1a1a1a]/60 hover:text-[#f5f5f5] hover:border-[#333333]",
  };

  const sizeClasses = {
    sm: "px-3 py-1.5 text-xs",
    md: "px-4 py-2 text-sm",
    lg: "px-5 py-2.5 text-base",
  };

  return (
    <button
      className={`${baseClasses} ${variantClasses[variant]} ${sizeClasses[size]} ${className}`}
      {...props}
    >
      {children}
    </button>
  );
};
