import React from "react";

interface SettingsGroupProps {
  title?: string;
  description?: string;
  children: React.ReactNode;
}

export const SettingsGroup: React.FC<SettingsGroupProps> = ({
  title,
  description,
  children,
}) => {
  return (
    <div className="space-y-4">
      {title && (
        <div className="px-1 pt-2">
          <h2 className="text-xs font-bold text-[#ff4d8d] uppercase tracking-widest">
            {title}
          </h2>
          {description && (
            <p className="text-xs text-[#a0a0a0] mt-1.5 leading-relaxed">{description}</p>
          )}
        </div>
      )}
      <div className="glass-panel-subtle rounded-xl overflow-visible border border-white/[0.03]">
        <div className="divide-y divide-white/[0.05]">{children}</div>
      </div>
    </div>
  );
};
