import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type MouseEvent,
  type PointerEvent,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";

const windowRef = getCurrentWindow();
const WINDOW_SIZE_COMPACT = { width: 80, height: 80 };
const WINDOW_SIZE_WITH_AOT = { width: 80, height: 108 };
const CLOSE_DRAG_THRESHOLD = 4;

interface SettingsChangedPayload {
  setting?: string;
  value?: unknown;
}

export default function VoiceActivationButton() {
  const [isPushToTalk, setIsPushToTalk] = useState(true);
  const [isRecording, setIsRecording] = useState(false);
  const [isAlwaysOnTop, setIsAlwaysOnTop] = useState(true);
  const [showAotToggle, setShowAotToggle] = useState(false);
  const [isSingleClickClose, setIsSingleClickClose] = useState(false);
  const [isBusy, setIsBusy] = useState(false);
  const closeButtonSuppressClickRef = useRef(false);
  const closeButtonDragRef = useRef({
    pointerId: -1,
    startX: 0,
    startY: 0,
    dragging: false,
  });

  const refreshPushToTalk = useCallback(async () => {
    try {
      const enabled = await invoke<boolean>("voice_activation_button_get_push_to_talk");
      setIsPushToTalk(enabled);
      return enabled;
    } catch (error) {
      console.error("Failed to read push-to-talk mode:", error);
      return true;
    }
  }, []);

  const refreshRecordingState = useCallback(async () => {
    try {
      const recording = await invoke<boolean>("is_recording");
      setIsRecording(recording);
      return recording;
    } catch (error) {
      console.error("Failed to read recording state:", error);
      return false;
    }
  }, []);

  const applyWindowMode = useCallback(async (showToggle: boolean) => {
    const size = showToggle ? WINDOW_SIZE_WITH_AOT : WINDOW_SIZE_COMPACT;
    await windowRef.setSize(new LogicalSize(size.width, size.height));

    if (showToggle) {
      const current = await windowRef.isAlwaysOnTop();
      setIsAlwaysOnTop(current);
    } else {
      await windowRef.setAlwaysOnTop(true);
      setIsAlwaysOnTop(true);
    }
  }, []);

  const refreshShowAotToggle = useCallback(async () => {
    try {
      const enabled = await invoke<boolean>(
        "voice_activation_button_get_show_aot_toggle",
      );
      setShowAotToggle(enabled);
      await applyWindowMode(enabled);
    } catch (error) {
      console.error("Failed to read voice button AOT toggle setting:", error);
      setShowAotToggle(false);
      await applyWindowMode(false);
    }
  }, [applyWindowMode]);

  const refreshSingleClickClose = useCallback(async () => {
    try {
      const enabled = await invoke<boolean>(
        "voice_activation_button_get_single_click_close",
      );
      setIsSingleClickClose(enabled);
    } catch (error) {
      console.error("Failed to read voice button close-click setting:", error);
      setIsSingleClickClose(false);
    }
  }, []);

  useEffect(() => {
    void refreshPushToTalk();
    void refreshRecordingState();
    void refreshShowAotToggle();
    void refreshSingleClickClose();

    const unlistenProfile = listen("active-profile-changed", () => {
      void refreshPushToTalk();
      void refreshRecordingState();
    });
    const unlistenSettings = listen<SettingsChangedPayload>(
      "settings-changed",
      (event) => {
        if (event.payload?.setting === "voice_button_show_aot_toggle") {
          void refreshShowAotToggle();
        } else if (
          event.payload?.setting === "voice_button_single_click_close"
        ) {
          void refreshSingleClickClose();
        }
      },
    );

    return () => {
      unlistenProfile.then((fn) => fn());
      unlistenSettings.then((fn) => fn());
    };
  }, [
    refreshPushToTalk,
    refreshRecordingState,
    refreshShowAotToggle,
    refreshSingleClickClose,
  ]);

  const handlePointerDown = async () => {
    if (isBusy || !isPushToTalk) return;

    setIsBusy(true);
    try {
      await invoke("voice_activation_button_press");
      await refreshRecordingState();
    } catch (error) {
      console.error("Failed to start recording:", error);
      setIsRecording(false);
    } finally {
      setIsBusy(false);
    }
  };

  const handlePointerRelease = async () => {
    if (isBusy || !isPushToTalk || !isRecording) return;

    setIsBusy(true);
    try {
      await invoke("voice_activation_button_release");
    } catch (error) {
      console.error("Failed to stop recording:", error);
    } finally {
      await refreshRecordingState();
      setIsBusy(false);
    }
  };

  const handleToggleModeClick = async () => {
    if (isBusy || isPushToTalk) return;

    setIsBusy(true);
    try {
      await invoke("voice_activation_button_press");
      await refreshRecordingState();
    } catch (error) {
      console.error("Failed to toggle recording:", error);
    } finally {
      setIsBusy(false);
    }
  };

  const toggleAlwaysOnTop = async () => {
    if (!showAotToggle) return;

    const next = !isAlwaysOnTop;
    try {
      await windowRef.setAlwaysOnTop(next);
      setIsAlwaysOnTop(next);
    } catch (error) {
      console.error("Failed to update always-on-top:", error);
    }
  };

  // Prevent drag when interacting with buttons
  const stopDragPropagation = (e: PointerEvent) => {
    e.stopPropagation();
  };

  const handleRootPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    // Drag from any non-control area; control buttons stop propagation.
    if (e.button !== 0) return;
    void windowRef.startDragging().catch((error) => {
      console.error("Failed to start window dragging (pointer):", error);
    });
  };

  const resetCloseButtonDragState = () => {
    closeButtonDragRef.current = {
      pointerId: -1,
      startX: 0,
      startY: 0,
      dragging: false,
    };
  };

  const handleCloseButtonPointerDown = (
    e: PointerEvent<HTMLButtonElement>,
  ) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    closeButtonSuppressClickRef.current = false;
    closeButtonDragRef.current = {
      pointerId: e.pointerId,
      startX: e.clientX,
      startY: e.clientY,
      dragging: false,
    };
  };

  const handleCloseButtonPointerMove = (
    e: PointerEvent<HTMLButtonElement>,
  ) => {
    e.stopPropagation();
    const dragState = closeButtonDragRef.current;
    if (dragState.pointerId !== e.pointerId || dragState.dragging) return;

    const distance = Math.hypot(
      e.clientX - dragState.startX,
      e.clientY - dragState.startY,
    );

    if (distance < CLOSE_DRAG_THRESHOLD) return;

    dragState.dragging = true;
    closeButtonSuppressClickRef.current = true;
    void windowRef.startDragging().catch((error) => {
      console.error("Failed to start window dragging from close button:", error);
    });
  };

  const handleCloseButtonPointerEnd = (
    e: PointerEvent<HTMLButtonElement>,
  ) => {
    e.stopPropagation();
    if (closeButtonDragRef.current.pointerId !== e.pointerId) return;
    resetCloseButtonDragState();
  };

  const closeWindow = () => {
    resetCloseButtonDragState();
    closeButtonSuppressClickRef.current = false;
    void windowRef.hide();
  };

  const handleCloseButtonClick = (e: MouseEvent<HTMLButtonElement>) => {
    e.stopPropagation();
    if (closeButtonSuppressClickRef.current) {
      closeButtonSuppressClickRef.current = false;
      return;
    }
    if (!isSingleClickClose) return;
    closeWindow();
  };

  const handleCloseButtonDoubleClick = (e: MouseEvent<HTMLButtonElement>) => {
    e.stopPropagation();
    if (isSingleClickClose) return;
    closeWindow();
  };

  return (
    <div
      className="voice-button-root"
      data-tauri-drag-region
      style={{ width: "100%", height: "100%" }}
      onPointerDown={handleRootPointerDown}
    >
      <div className="main-controls">
        <button
          type="button"
          className={`voice-button ${isRecording ? "recording" : ""}`}
          onPointerDown={(e) => {
            stopDragPropagation(e);
            void handlePointerDown();
          }}
          onPointerUp={handlePointerRelease}
          onPointerLeave={handlePointerRelease}
          onClick={handleToggleModeClick}
          disabled={isBusy}
        >
          <span className="voice-dot" />
        </button>
        <button
          type="button"
          className="close-button"
          title={
            isSingleClickClose
              ? "Drag to move. Click once to close."
              : "Drag to move. Double-click to close."
          }
          onPointerDown={handleCloseButtonPointerDown}
          onPointerMove={handleCloseButtonPointerMove}
          onPointerUp={handleCloseButtonPointerEnd}
          onPointerCancel={handleCloseButtonPointerEnd}
          onClick={handleCloseButtonClick}
          onDoubleClick={handleCloseButtonDoubleClick}
        >
          <span className="close-button-icons" aria-hidden="true">
            <span className="close-button-arrows">↕</span>
            <span className="close-button-x">x</span>
          </span>
        </button>
      </div>
      {showAotToggle && (
        <button
          type="button"
          className={`always-on-top-toggle ${isAlwaysOnTop ? "on" : "off"}`}
          onPointerDown={stopDragPropagation}
          onClick={toggleAlwaysOnTop}
        >
          AOT {isAlwaysOnTop ? "On" : "Off"}
        </button>
      )}
    </div>
  );
}
