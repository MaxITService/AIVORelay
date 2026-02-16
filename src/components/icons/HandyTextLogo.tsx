import { exit } from "@tauri-apps/plugin-process";
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

  const handleDevModeDoubleClick = async () => {
    try {
      await exit(0);
    } catch (err) {
      console.error("Failed to exit app:", err);
    }
  };

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
        <span
          className="text-xs font-semibold text-orange-400 mt-1 cursor-pointer select-none hover:text-orange-300 transition-colors"
          onDoubleClick={handleDevModeDoubleClick}
          title="Double click to exit app"
        >
          Dev Mode
        </span>
      )}
    </div>
  );
};

export default HandyTextLogo;
