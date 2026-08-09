import React, { useEffect, useRef, useState } from "react";

import { Input } from "@/components/ui/Input";

type CommittedNumberInputProps = Omit<
  React.ComponentProps<typeof Input>,
  | "type"
  | "value"
  | "defaultValue"
  | "onChange"
  | "onBlur"
  | "onKeyDown"
  | "min"
  | "max"
  | "step"
> & {
  value: number;
  min: number;
  max: number;
  step?: number;
  onCommit: (value: number) => void | Promise<void>;
};

/**
 * Keeps incomplete number edits local so clearing a field or typing a decimal
 * does not immediately clamp and persist an unintended value.
 */
export const CommittedNumberInput: React.FC<CommittedNumberInputProps> = ({
  value,
  min,
  max,
  step = 1,
  onCommit,
  onFocus,
  ...props
}) => {
  const [draft, setDraft] = useState(String(value));
  const [focused, setFocused] = useState(false);
  const skipBlurCommitRef = useRef(false);

  useEffect(() => {
    if (!focused) {
      setDraft(String(value));
    }
  }, [focused, value]);

  const commitDraft = () => {
    const parsed = Number(draft);
    if (!draft.trim() || !Number.isFinite(parsed)) {
      setDraft(String(value));
      return;
    }

    const bounded = Math.min(max, Math.max(min, parsed));
    const nextValue = Number.isInteger(step) ? Math.round(bounded) : bounded;
    setDraft(String(nextValue));
    if (nextValue !== value) {
      void onCommit(nextValue);
    }
  };

  return (
    <Input
      {...props}
      type="number"
      min={min}
      max={max}
      step={step}
      value={draft}
      inputMode={Number.isInteger(step) ? "numeric" : "decimal"}
      onFocus={(event) => {
        setFocused(true);
        onFocus?.(event);
      }}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={() => {
        setFocused(false);
        if (skipBlurCommitRef.current) {
          skipBlurCommitRef.current = false;
          return;
        }
        commitDraft();
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          event.preventDefault();
          event.currentTarget.blur();
        } else if (event.key === "Escape") {
          event.preventDefault();
          skipBlurCommitRef.current = true;
          setDraft(String(value));
          event.currentTarget.blur();
        }
      }}
    />
  );
};
