import React from "react";

// Microphone in front of a processor chip (Speech Processing). Drawn on the
// Lucide 24px grid with Lucide's stroke style and the same chip size as Lucide's
// Cpu, so it matches the other sidebar icons. The pins are a little longer
// than Cpu's to read as a chip at 20px; they run clockwise from the top left.
const SpeechProcessingIcon: React.FC<React.SVGProps<SVGSVGElement>> = ({
  width = 24,
  height = 24,
  className = "",
  children,
  ...rest
}) => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    width={width}
    height={height}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth={2}
    strokeLinecap="round"
    strokeLinejoin="round"
    className={`speech-processing-icon ${className}`.trim()}
    {...rest}
  >
    <rect
      className="speech-processing-icon__chip"
      x="4"
      y="4"
      width="16"
      height="16"
      rx="2"
    />
    <g className="speech-processing-icon__pins">
      <path className="speech-processing-icon__pin" d="M8 1.5V4" />
      <path className="speech-processing-icon__pin" d="M16 1.5V4" />
      <path className="speech-processing-icon__pin" d="M20 8h2.5" />
      <path className="speech-processing-icon__pin" d="M20 16h2.5" />
      <path className="speech-processing-icon__pin" d="M16 20v2.5" />
      <path className="speech-processing-icon__pin" d="M8 20v2.5" />
      <path className="speech-processing-icon__pin" d="M1.5 16H4" />
      <path className="speech-processing-icon__pin" d="M1.5 8H4" />
    </g>
    <g className="speech-processing-icon__mic">
      <rect x="9.5" y="7" width="5" height="7.5" rx="2.5" />
      <path d="M7 12v.5a5 5 0 0 0 10 0V12" />
      <path d="M12 17.5V20" />
    </g>
    {children}
  </svg>
);

export default SpeechProcessingIcon;
