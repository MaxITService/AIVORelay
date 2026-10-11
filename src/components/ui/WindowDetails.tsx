import React from "react";
import { useWindowState } from "@/hooks/useWindowState";

type Props = Omit<React.ComponentPropsWithoutRef<"details">, "open"> & {
  stateKey: string;
};

export const WindowDetails: React.FC<Props> = ({ stateKey, onToggle, ...props }) => {
  const [open, setOpen] = useWindowState(stateKey, false);

  return (
    <details
      {...props}
      open={open}
      onToggle={(event) => {
        setOpen(event.currentTarget.open);
        onToggle?.(event);
      }}
    />
  );
};
