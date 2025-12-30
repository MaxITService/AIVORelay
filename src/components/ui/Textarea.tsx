import React from "react";

interface TextareaProps
  extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  variant?: "default" | "compact";
}

export const Textarea: React.FC<TextareaProps> = ({
  className = "",
  variant = "default",
  ...props
}) => {
  const baseClasses =
    "text-sm font-medium bg-[#121212]/80 border border-[#333333] rounded-md text-[#f5f5f5] placeholder-[#707070] transition-all duration-200 hover:border-[#3d3d3d] hover:bg-[#1a1a1a]/80 focus:outline-none focus:border-[#ff4d8d] focus:shadow-[0_0_0_2px_rgba(255,77,141,0.2)] resize-y";

  const variantClasses = {
    default: "px-3 py-2.5 min-h-[100px]",
    compact: "px-2 py-1.5 min-h-[80px]",
  };

  return (
    <textarea
      className={`${baseClasses} ${variantClasses[variant]} ${className}`}
      {...props}
    />
  );
};
