import React, { useCallback, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { sessionToast as toast } from "@/lib/sessionToast";
import { prefersReducedMotion } from "@/lib/motion";
import { useSettings } from "../../hooks/useSettings";
import { Input } from "../ui/Input";
import { Button } from "../ui/Button";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { SettingContainer } from "../ui/SettingContainer";
import "./CustomWords.css";

interface CustomWordsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

interface LayoutSnapshot {
  chips: Map<string, DOMRect>;
  containerHeight: number;
}

interface PoppingChipData {
  id: number;
  word: string;
  left: number;
  top: number;
  width: number;
  height: number;
}

const CHIP_LAYOUT_DURATION_MS = 380;
const CHIP_LAYOUT_EASING = "cubic-bezier(0.22, 1, 0.36, 1)";
const CHIP_SLIDE_EASING = "cubic-bezier(0.3, 1.2, 0.5, 1)";
// Lets the removed chip start shrinking before neighbors move in
const CHIP_SLIDE_DELAY_MS = 60;

const CHIP_ENTER_DURATION_MS = 560;
const CHIP_ENTER_KEYFRAMES: Keyframe[] = [
  {
    transform: "scale(0.2)",
    opacity: 0,
    easing: "cubic-bezier(0.22, 1, 0.36, 1)",
  },
  { transform: "scale(1.12)", opacity: 1, offset: 0.45, easing: "ease-in-out" },
  { transform: "scale(0.95)", offset: 0.72, easing: "ease-in-out" },
  { transform: "scale(1.02)", offset: 0.88, easing: "ease-in-out" },
  { transform: "scale(1)" },
];

const CHIP_POP_DURATION_MS = 240;
const CHIP_POP_KEYFRAMES: Keyframe[] = [
  {
    transform: "scale(1)",
    opacity: 1,
    easing: "cubic-bezier(0.55, 0, 0.8, 0.3)",
  },
  { transform: "scale(0.3)", opacity: 0.9, offset: 0.8 },
  { transform: "scale(0)", opacity: 0 },
];
const CHIP_BURST_DELAY_MS = 170;
// Covers the longest burst animation in CustomWords.css
const CHIP_POP_LIFETIME_MS = CHIP_BURST_DELAY_MS + 480;
const SPARK_ANGLES = [15, 75, 135, 195, 255, 315];

const CHIP_SHAKE_DURATION_MS = 520;
const CHIP_SHAKE_KEYFRAMES: Keyframe[] = [
  { transform: "translateX(0) rotate(0deg)" },
  { transform: "translateX(-5px) rotate(-2deg)", offset: 0.12 },
  { transform: "translateX(5px) rotate(2deg)", offset: 0.26 },
  { transform: "translateX(-4px) rotate(-1.5deg)", offset: 0.4 },
  { transform: "translateX(4px) rotate(1.5deg)", offset: 0.54 },
  { transform: "translateX(-2px) rotate(0deg)", offset: 0.7 },
  { transform: "translateX(2px) rotate(0deg)", offset: 0.85 },
  { transform: "translateX(0) rotate(0deg)" },
];

const CHIP_HIGHLIGHT_DURATION_MS = 1500;
const CHIP_HIGHLIGHT_STYLE = {
  borderColor: "#ff4d8d",
  backgroundColor: "rgba(255, 77, 141, 0.2)",
  boxShadow:
    "0 0 0 2px rgba(255, 77, 141, 0.35), 0 0 18px rgba(255, 77, 141, 0.5)",
};
// Start and end keyframes are implicit, so the chip returns to its own style
const CHIP_HIGHLIGHT_KEYFRAMES: Keyframe[] = [
  { offset: 0.1, ...CHIP_HIGHLIGHT_STYLE },
  { offset: 0.65, ...CHIP_HIGHLIGHT_STYLE },
];

const normalizeCustomWord = (word: string) =>
  word
    .replace(/[<>"']/g, "")
    .replace(/\s+/g, " ")
    .trim();

const animateContainerHeight = (
  container: HTMLElement,
  fromHeight: number,
  collapse: boolean,
) => {
  container.getAnimations().forEach((animation) => animation.cancel());
  const options: KeyframeAnimationOptions = {
    duration: CHIP_LAYOUT_DURATION_MS,
    easing: CHIP_LAYOUT_EASING,
  };

  if (collapse) {
    // Last word removed: fold the list away while its chip pops
    container.animate(
      [
        { height: `${fromHeight}px` },
        { height: "0px", paddingTop: "0px", paddingBottom: "0px" },
      ],
      { ...options, fill: "forwards" },
    );
    return;
  }

  const toHeight = container.getBoundingClientRect().height;
  if (Math.abs(toHeight - fromHeight) < 0.5) return;
  const animation = container.animate(
    [{ height: `${fromHeight}px` }, { height: `${toHeight}px` }],
    options,
  );
  if (toHeight > fromHeight) {
    // Keep a chip that opens a new row inside the list while it grows
    container.style.overflow = "hidden";
    const restoreOverflow = () => {
      container.style.overflow = "";
    };
    animation.addEventListener("finish", restoreOverflow);
    animation.addEventListener("cancel", restoreOverflow);
  }
};

const WordChipLabel: React.FC<{ word: string }> = ({ word }) => (
  <>
    <span>{word}</span>
    <svg
      className="aivo-word-chip__x w-3 h-3"
      fill="none"
      stroke="currentColor"
      viewBox="0 0 24 24"
    >
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth={2}
        d="M6 18L18 6M6 6l12 12"
      />
    </svg>
  </>
);

const PoppingChip: React.FC<{
  chip: PoppingChipData;
  onDone: (id: number) => void;
}> = ({ chip, onDone }) => {
  const bodyRef = useRef<HTMLSpanElement>(null);

  useLayoutEffect(() => {
    const animation = bodyRef.current?.animate(CHIP_POP_KEYFRAMES, {
      duration: CHIP_POP_DURATION_MS,
      fill: "forwards",
    });
    const timer = window.setTimeout(
      () => onDone(chip.id),
      CHIP_POP_LIFETIME_MS,
    );
    return () => {
      animation?.cancel();
      window.clearTimeout(timer);
    };
  }, [chip.id, onDone]);

  return (
    <span
      className="aivo-word-chip aivo-word-chip--popping"
      style={{
        left: chip.left,
        top: chip.top,
        width: chip.width,
        height: chip.height,
      }}
      aria-hidden="true"
    >
      <span ref={bodyRef} className="aivo-word-chip__body">
        <Button
          variant="secondary"
          size="sm"
          tabIndex={-1}
          className="aivo-word-chip__button inline-flex items-center gap-1"
        >
          <WordChipLabel word={chip.word} />
        </Button>
      </span>
      <span
        className="aivo-word-chip__burst"
        style={
          {
            "--aivo-word-chip-burst-delay": `${CHIP_BURST_DELAY_MS}ms`,
          } as React.CSSProperties
        }
      >
        <span className="aivo-word-chip__flash" />
        <span className="aivo-word-chip__ring" />
        {SPARK_ANGLES.map((angle, index) => (
          <span
            key={angle}
            className="aivo-word-chip__spark"
            data-alt={index % 2 === 1}
            style={
              {
                "--aivo-word-chip-spark-angle": `${angle}deg`,
              } as React.CSSProperties
            }
          />
        ))}
      </span>
    </span>
  );
};

export const CustomWords: React.FC<CustomWordsProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const [newWord, setNewWord] = useState("");
    const [poppingChips, setPoppingChips] = useState<PoppingChipData[]>([]);
    const containerRef = useRef<HTMLDivElement>(null);
    const chipRefs = useRef(new Map<string, HTMLSpanElement>());
    const layoutSnapshotRef = useRef<LayoutSnapshot | null>(null);
    const enteringWordRef = useRef<string | null>(null);
    const highlightAnimationsRef = useRef<Animation[]>([]);
    const popIdRef = useRef(0);
    const customWords = getSetting("custom_words") || [];
    const customWordsEnabled = getSetting("custom_words_enabled");
    const isCustomWordsEnabled = customWordsEnabled ?? true;
    const normalizedWord = normalizeCustomWord(newWord);

    // Records where chips are before a change so the next layout can animate from there
    const captureLayout = () => {
      const chips = new Map<string, DOMRect>();
      chipRefs.current.forEach((chip, word) =>
        chips.set(word, chip.getBoundingClientRect()),
      );
      layoutSnapshotRef.current = {
        chips,
        containerHeight:
          containerRef.current?.getBoundingClientRect().height ?? 0,
      };
    };

    useLayoutEffect(() => {
      const snapshot = layoutSnapshotRef.current;
      const enteringWord = enteringWordRef.current;
      layoutSnapshotRef.current = null;
      enteringWordRef.current = null;
      const container = containerRef.current;
      if (!snapshot || !container) return;

      // Slide chips that moved from their previous spot to the new one
      chipRefs.current.forEach((chip, word) => {
        const before = snapshot.chips.get(word);
        if (!before) return;
        chip.getAnimations().forEach((animation) => animation.cancel());
        const after = chip.getBoundingClientRect();
        const dx = before.left - after.left;
        const dy = before.top - after.top;
        if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5) return;
        chip.animate(
          [
            { transform: `translate(${dx}px, ${dy}px)` },
            { transform: "translate(0, 0)" },
          ],
          {
            duration: CHIP_LAYOUT_DURATION_MS,
            delay: CHIP_SLIDE_DELAY_MS,
            easing: CHIP_SLIDE_EASING,
            fill: "backwards",
          },
        );
      });

      animateContainerHeight(
        container,
        snapshot.containerHeight,
        chipRefs.current.size === 0,
      );

      if (enteringWord) {
        chipRefs.current
          .get(enteringWord)
          ?.querySelector(".aivo-word-chip__body")
          ?.animate(CHIP_ENTER_KEYFRAMES, { duration: CHIP_ENTER_DURATION_MS });
      }
    }, [customWords]);

    const highlightExistingWord = (word: string) => {
      const chip = chipRefs.current.get(word);
      const body = chip?.querySelector(".aivo-word-chip__body");
      const button = chip?.querySelector(".aivo-word-chip__button");
      if (!chip || !body || !button) return;

      const reduceMotion = prefersReducedMotion();
      highlightAnimationsRef.current.forEach((animation) => animation.cancel());
      chip.scrollIntoView({
        block: "nearest",
        inline: "nearest",
        behavior: reduceMotion ? "auto" : "smooth",
      });
      highlightAnimationsRef.current = [
        button.animate(CHIP_HIGHLIGHT_KEYFRAMES, {
          duration: CHIP_HIGHLIGHT_DURATION_MS,
          easing: "ease-out",
        }),
      ];
      if (!reduceMotion) {
        highlightAnimationsRef.current.push(
          body.animate(CHIP_SHAKE_KEYFRAMES, {
            duration: CHIP_SHAKE_DURATION_MS,
            easing: "ease-out",
          }),
        );
      }
    };

    const handlePopDone = useCallback((id: number) => {
      setPoppingChips((chips) => chips.filter((chip) => chip.id !== id));
    }, []);

    const handleAddWord = () => {
      if (normalizedWord && normalizedWord.length <= 50) {
        if (customWords.includes(normalizedWord)) {
          highlightExistingWord(normalizedWord);
          toast.error(
            t("settings.advanced.customWords.duplicate", {
              word: normalizedWord,
            }),
          );
          return;
        }
        if (!prefersReducedMotion()) {
          captureLayout();
          enteringWordRef.current = normalizedWord;
        }
        updateSetting("custom_words", [...customWords, normalizedWord]);
        setNewWord("");
      }
    };

    const handleRemoveWord = (wordToRemove: string) => {
      // Chips stay visually enabled during the short save to avoid flicker
      if (isUpdating("custom_words")) return;
      const container = containerRef.current;
      const chip = chipRefs.current.get(wordToRemove);
      if (container && chip && !prefersReducedMotion()) {
        captureLayout();
        const containerRect = container.getBoundingClientRect();
        const chipRect = chip.getBoundingClientRect();
        popIdRef.current += 1;
        const poppingChip: PoppingChipData = {
          id: popIdRef.current,
          word: wordToRemove,
          left: chipRect.left - containerRect.left - container.clientLeft,
          top: chipRect.top - containerRect.top - container.clientTop,
          width: chipRect.width,
          height: chipRect.height,
        };
        setPoppingChips((chips) => [...chips, poppingChip]);
      }
      updateSetting(
        "custom_words",
        customWords.filter((word) => word !== wordToRemove),
      );
    };

    const handleKeyPress = (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleAddWord();
      }
    };

    return (
      <>
        <ToggleSwitch
          checked={isCustomWordsEnabled}
          onChange={(enabled) =>
            updateSetting("custom_words_enabled", enabled)
          }
          isUpdating={isUpdating("custom_words_enabled")}
          label={t(
            "settings.advanced.customWords.enabledLabel",
            "Enable Custom Words",
          )}
          description={t(
            "settings.advanced.customWords.enabledDescription",
            "Applies to live transcription. File transcription has its own toggle.",
          )}
          descriptionMode={descriptionMode}
          grouped={grouped}
        />
        <SettingContainer
          title={t("settings.advanced.customWords.title")}
          description={t("settings.advanced.customWords.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <div className="flex items-center gap-2">
            <Input
              type="text"
              className="max-w-40"
              value={newWord}
              onChange={(e) => setNewWord(e.target.value)}
              onKeyDown={handleKeyPress}
              placeholder={t("settings.advanced.customWords.placeholder")}
              variant="compact"
              disabled={isUpdating("custom_words")}
            />
            <Button
              onClick={handleAddWord}
              disabled={
                !normalizedWord ||
                normalizedWord.length > 50 ||
                isUpdating("custom_words")
              }
              variant="primary"
              size="md"
            >
              {t("settings.advanced.customWords.add")}
            </Button>
          </div>
        </SettingContainer>
        {(customWords.length > 0 || poppingChips.length > 0) && (
          <div
            ref={containerRef}
            className={`relative px-4 p-2 ${grouped ? "" : "rounded-lg border border-mid-gray/20"} flex flex-wrap gap-1`}
          >
            {customWords.map((word) => (
              <span
                key={word}
                ref={(chip) => {
                  if (chip) chipRefs.current.set(word, chip);
                  else chipRefs.current.delete(word);
                }}
                className="aivo-word-chip"
              >
                <span className="aivo-word-chip__body">
                  <Button
                    onClick={() => handleRemoveWord(word)}
                    variant="secondary"
                    size="sm"
                    className="aivo-word-chip__button inline-flex items-center gap-1 cursor-pointer"
                    aria-label={t("settings.advanced.customWords.remove", {
                      word,
                    })}
                  >
                    <WordChipLabel word={word} />
                  </Button>
                </span>
              </span>
            ))}
            {poppingChips.map((chip) => (
              <PoppingChip key={chip.id} chip={chip} onDone={handlePopDone} />
            ))}
          </div>
        )}
      </>
    );
  },
);
