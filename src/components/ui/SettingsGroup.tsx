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
    <div className="space-y-3">
      {title && (
        <div className="px-1">
          <h2 className="text-xs font-semibold text-[#ff4d8d] uppercase tracking-wider">
            {title}
          </h2>
          {description && (
            <p className="text-xs text-[#6b6b6b] mt-1">{description}</p>
          )}
        </div>
      )}
      <div className="glass-panel-subtle rounded-xl overflow-visible">
        <div className="divide-y divide-[#2f2f2f]">{children}</div>
      </div>
    </div>
  );
};
