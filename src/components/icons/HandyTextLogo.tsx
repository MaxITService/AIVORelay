import React from "react";
import largeLogoUrl from "../../assets/large_logo.jpg";

const HandyTextLogo = ({
  width,
  height,
  className,
}: {
  width?: number;
  height?: number;
  className?: string;
}) => {
  const resolvedWidth = width ?? (height ? undefined : 300);

  return (
    <div className="flex flex-col items-center">
      <img
        src={largeLogoUrl}
        alt="AivoRelay"
        width={resolvedWidth}
        height={height}
        className={className}
      />
      {import.meta.env.DEV && (
        <span className="text-xs font-semibold text-orange-400 mt-1">
          Dev Mode
        </span>
      )}
    </div>
  );
};

export default HandyTextLogo;
