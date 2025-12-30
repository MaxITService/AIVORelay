import React from "react";

interface BadgeProps {
  children: React.ReactNode;
  variant?: "primary" | "secondary" | "success" | "warning";
  className?: string;
}

const Badge: React.FC<BadgeProps> = ({
  children,
  variant = "primary",
  className = "",
}) => {
  const variantClasses = {
    primary: "bg-gradient-to-r from-[#ff4d8d] to-[#9b5de5] text-white",
    secondary: "bg-[#1a1a1a] border border-[#333333] text-[#f5f5f5]",
    success: "bg-gradient-to-r from-[#ffd93d] to-[#ffb347] text-[#121212]",
    warning: "bg-gradient-to-r from-[#ffb347] to-[#ff8c42] text-[#121212]",
  };

  return (
    <span
      className={`inline-flex items-center px-3 py-1 rounded-full text-xs font-semibold shadow-sm ${variantClasses[variant]} ${className}`}
    >
      {children}
    </span>
  );
};

export default Badge;
