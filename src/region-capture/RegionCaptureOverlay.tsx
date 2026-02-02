import { useEffect, useState, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

// State machine states
type CaptureState = "idle" | "creating" | "selected" | "moving" | "resizing";

// Resize handle positions
type HandlePosition = "nw" | "n" | "ne" | "w" | "e" | "sw" | "s" | "se";

interface Region {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface VirtualScreenInfo {
  offset_x: number;
  offset_y: number;
  total_width: number;
  total_height: number;
  scale_factor: number;
}

interface RegionCaptureData {
  screenshot: string | null; // base64 (legacy mode only)
  virtual_screen: VirtualScreenInfo;
}

const MIN_REGION_SIZE = 10;
const HANDLE_SIZE = 10;

export default function RegionCaptureOverlay() {
  const [screenshot, setScreenshot] = useState<string | null>(null);
  const [virtualScreen, setVirtualScreen] = useState<VirtualScreenInfo | null>(null);
  const [state, setState] = useState<CaptureState>("idle");
  const [region, setRegion] = useState<Region | null>(null);
  const [activeHandle, setActiveHandle] = useState<HandlePosition | null>(null);
  const [dragStart, setDragStart] = useState<{ x: number; y: number } | null>(null);
  const [regionStart, setRegionStart] = useState<Region | null>(null);
  const [error, setError] = useState<string | null>(null);

  const containerRef = useRef<HTMLDivElement>(null);

  // For double-click handling: save region before click sequence starts
  const savedRegionRef = useRef<Region | null>(null);
  const lastMouseDownTime = useRef<number>(0);

  // Fetch data from backend when component mounts
  useEffect(() => {
    const fetchData = async () => {
        try {
          const data = await invoke<RegionCaptureData>("region_capture_get_data");
        setScreenshot(data.screenshot ? `data:image/png;base64,${data.screenshot}` : null);
        setVirtualScreen(data.virtual_screen);
      } catch (e) {
        console.error("Failed to get region capture data:", e);
        setError(String(e));
      }
    };
    fetchData();
  }, []);

  // Confirm region selection (used by Enter key and double-click)
  const handleConfirm = useCallback((regionToConfirm?: Region) => {
    const r = regionToConfirm || region;
    if (r && virtualScreen && r.width > MIN_REGION_SIZE && r.height > MIN_REGION_SIZE) {
      const scale = virtualScreen.scale_factor || 1;
      invoke("region_capture_confirm", {
        region: {
          x: Math.round(r.x * scale),
          y: Math.round(r.y * scale),
          width: Math.round(r.width * scale),
          height: Math.round(r.height * scale),
        },
      });
    }
  }, [region, virtualScreen]);

  // Handle double-click: confirm saved selection or send full screen
  const handleDoubleClick = useCallback(
    () => {
      if (!virtualScreen) return;

      // Use the region saved at the START of this click sequence
      const savedRegion = savedRegionRef.current;

      // Clear visible region before confirming to avoid purple frame in screenshot
      setRegion(null);

      // Wait for next frame to ensure UI update before sending confirm
      requestAnimationFrame(() => {
        if (savedRegion && savedRegion.width > MIN_REGION_SIZE && savedRegion.height > MIN_REGION_SIZE) {
          // Had a valid selection before double-click → send it
          handleConfirm(savedRegion);
        } else {
          // No valid selection → send full screen
          const fullScreen: Region = {
            x: 0,
            y: 0,
            width: virtualScreen.total_width / (virtualScreen.scale_factor || 1),
            height: virtualScreen.total_height / (virtualScreen.scale_factor || 1),
          };
          handleConfirm(fullScreen);
        }
      });
    },
    [virtualScreen, handleConfirm]
  );

  // Handle keyboard events
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        invoke("region_capture_cancel");
      } else if (e.key === "Enter") {
        e.preventDefault();
        handleConfirm();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleConfirm]);

  // Get handle at a point
  const getHandleAtPoint = useCallback(
    (x: number, y: number): HandlePosition | null => {
      if (!region) return null;

      const handles: Record<HandlePosition, { x: number; y: number }> = {
        nw: { x: region.x, y: region.y },
        n: { x: region.x + region.width / 2, y: region.y },
        ne: { x: region.x + region.width, y: region.y },
        w: { x: region.x, y: region.y + region.height / 2 },
        e: { x: region.x + region.width, y: region.y + region.height / 2 },
        sw: { x: region.x, y: region.y + region.height },
        s: { x: region.x + region.width / 2, y: region.y + region.height },
        se: { x: region.x + region.width, y: region.y + region.height },
      };

      for (const [name, pos] of Object.entries(handles)) {
        if (Math.abs(x - pos.x) < HANDLE_SIZE && Math.abs(y - pos.y) < HANDLE_SIZE) {
          return name as HandlePosition;
        }
      }
      return null;
    },
    [region]
  );

  // Check if point is inside region
  const isPointInRegion = useCallback(
    (x: number, y: number): boolean => {
      if (!region) return false;
      return (
        x >= region.x &&
        x <= region.x + region.width &&
        y >= region.y &&
        y <= region.y + region.height
      );
    },
    [region]
  );

  // Mouse down handler
  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      const rect = containerRef.current?.getBoundingClientRect();
      if (!rect) return;

      // Save region at start of new click sequence (for double-click handling)
      const now = Date.now();
      if (now - lastMouseDownTime.current > 400) {
        savedRegionRef.current = region;
      }
      lastMouseDownTime.current = now;

      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;

      setDragStart({ x, y });

      if (state === "idle") {
        // Start creating new region
        setState("creating");
        setRegion({ x, y, width: 0, height: 0 });
      } else if (state === "selected") {
        const handle = getHandleAtPoint(x, y);
        if (handle) {
          // Start resizing
          setState("resizing");
          setActiveHandle(handle);
          setRegionStart(region ? { ...region } : null);
        } else if (isPointInRegion(x, y)) {
          // Start moving
          setState("moving");
          setRegionStart(region ? { ...region } : null);
        } else {
          // Click outside - start new region
          setState("creating");
          setRegion({ x, y, width: 0, height: 0 });
        }
      }
    },
    [state, region, getHandleAtPoint, isPointInRegion]
  );

  // Mouse move handler
  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      const rect = containerRef.current?.getBoundingClientRect();
      if (!rect || !dragStart) return;

      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;

      if (state === "creating") {
        // Update region while creating
        const newRegion = {
          x: Math.min(dragStart.x, x),
          y: Math.min(dragStart.y, y),
          width: Math.abs(x - dragStart.x),
          height: Math.abs(y - dragStart.y),
        };
        setRegion(newRegion);
      } else if (state === "moving" && regionStart) {
        // Move region
        const deltaX = x - dragStart.x;
        const deltaY = y - dragStart.y;
        setRegion({
          ...regionStart,
          x: regionStart.x + deltaX,
          y: regionStart.y + deltaY,
        });
      } else if (state === "resizing" && regionStart && activeHandle) {
        // Resize based on handle
        const deltaX = x - dragStart.x;
        const deltaY = y - dragStart.y;
        const newRegion = { ...regionStart };

        // Horizontal component
        if (activeHandle.includes("w")) {
          newRegion.x = regionStart.x + deltaX;
          newRegion.width = regionStart.width - deltaX;
        } else if (activeHandle.includes("e")) {
          newRegion.width = regionStart.width + deltaX;
        }

        // Vertical component
        if (activeHandle.includes("n")) {
          newRegion.y = regionStart.y + deltaY;
          newRegion.height = regionStart.height - deltaY;
        } else if (activeHandle.includes("s")) {
          newRegion.height = regionStart.height + deltaY;
        }

        // Enforce minimum size
        if (newRegion.width < MIN_REGION_SIZE) {
          if (activeHandle.includes("w")) {
            newRegion.x = regionStart.x + regionStart.width - MIN_REGION_SIZE;
          }
          newRegion.width = MIN_REGION_SIZE;
        }
        if (newRegion.height < MIN_REGION_SIZE) {
          if (activeHandle.includes("n")) {
            newRegion.y = regionStart.y + regionStart.height - MIN_REGION_SIZE;
          }
          newRegion.height = MIN_REGION_SIZE;
        }

        setRegion(newRegion);
      }
    },
    [state, dragStart, regionStart, activeHandle]
  );

  // Mouse up handler
  const handleMouseUp = useCallback(() => {
    if (state === "creating") {
      if (region && region.width > MIN_REGION_SIZE && region.height > MIN_REGION_SIZE) {
        setState("selected");
      } else {
        // Region too small - reset
        setRegion(null);
        setState("idle");
      }
    } else if (state === "moving" || state === "resizing") {
      setState("selected");
    }

    setDragStart(null);
    setRegionStart(null);
    setActiveHandle(null);
  }, [state, region]);

  // Calculate dim overlay positions
  const getDimStyles = () => {
    if (!region) {
      return {
        top: { height: "100%" },
        bottom: { height: 0 },
        left: { top: 0, height: 0, width: 0 },
        right: { top: 0, height: 0, width: 0 },
      };
    }

    return {
      top: { height: region.y },
      bottom: { top: region.y + region.height, height: `calc(100% - ${region.y + region.height}px)` },
      left: { top: region.y, height: region.height, width: region.x },
      right: { top: region.y, height: region.height, left: region.x + region.width },
    };
  };

  const dimStyles = getDimStyles();

  // Get container class based on state
  const getContainerClass = () => {
    let cls = "region-capture-container";
    if (state === "moving") cls += " state-moving";
    if (state === "resizing" && activeHandle) cls += ` state-resizing-${activeHandle}`;
    return cls;
  };

  // Get hint text based on state
  const getHintText = () => {
    if (error) return `Error: ${error}`;
    if (state === "idle") return "Click and drag to select a region, or double-click for full screen";
    if (state === "creating") return "Release to finish selection";
    if (state === "selected") return "Drag to move, use handles to resize. Enter or double-click to confirm, Escape to cancel";
    if (state === "moving") return "Release to finish moving";
    if (state === "resizing") return "Release to finish resizing";
    return "";
  };

  return (
    <div
      ref={containerRef}
      className={getContainerClass()}
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      onMouseLeave={handleMouseUp}
      onDoubleClick={handleDoubleClick}
    >
      {/* Optional screenshot background (legacy mode). When absent, desktop shows through. */}
      {screenshot && (
        <img src={screenshot} alt="" className="screenshot-background" draggable={false} />
      )}

      {/* Dim overlays */}
      <div className="dim-overlay dim-top" style={dimStyles.top} />
      <div className="dim-overlay dim-bottom" style={dimStyles.bottom} />
      <div className="dim-overlay dim-left" style={dimStyles.left} />
      <div className="dim-overlay dim-right" style={dimStyles.right} />

      {/* Selection region with handles */}
      {region && region.width > 0 && region.height > 0 && (
        <div
          className="selection-region"
          style={{
            left: region.x,
            top: region.y,
            width: region.width,
            height: region.height,
          }}
        >
          {state === "selected" && (
            <>
              <div className="resize-handle handle-nw" />
              <div className="resize-handle handle-n" />
              <div className="resize-handle handle-ne" />
              <div className="resize-handle handle-w" />
              <div className="resize-handle handle-e" />
              <div className="resize-handle handle-sw" />
              <div className="resize-handle handle-s" />
              <div className="resize-handle handle-se" />
            </>
          )}
        </div>
      )}

      {/* Hint text */}
      <div className="hint-text">{getHintText()}</div>
    </div>
  );
}
