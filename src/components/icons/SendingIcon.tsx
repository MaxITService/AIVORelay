import React from "react";

interface SendingIconProps {
  width?: number;
  height?: number;
  color?: string;
  className?: string;
}

const SendingIcon: React.FC<SendingIconProps> = ({
  width = 24,
  height = 24,
  color = "#FAA2CA",
  className = "",
}) => {
  return (
    <svg
      width={width}
      height={height}
      viewBox="0 0 24 24"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
    >
      {/* Paper plane / send icon - filled path */}
      <path
        d="M2.01 21L23 12L2.01 3L2 10L17 12L2 14L2.01 21Z"
        fill={color}
      />
    </svg>
  );
};

export default SendingIcon;
