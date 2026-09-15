export interface Point {
  x: number;
  y: number;
}

export interface Bounds {
  width: number;
  height: number;
}

export interface Region {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface VirtualScreenGeometry {
  total_width: number;
  total_height: number;
  scale_factor: number;
}

export type HandlePosition =
  | "nw"
  | "n"
  | "ne"
  | "w"
  | "e"
  | "sw"
  | "s"
  | "se";

export const MIN_REGION_SIZE = 10;

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}

function normalizedBounds(bounds: Bounds): Bounds {
  return {
    width: Math.max(0, bounds.width),
    height: Math.max(0, bounds.height),
  };
}

export function clampRegionToBounds(region: Region, bounds: Bounds): Region {
  const normalized = normalizedBounds(bounds);
  const width = clamp(region.width, 0, normalized.width);
  const height = clamp(region.height, 0, normalized.height);

  return {
    x: clamp(region.x, 0, normalized.width - width),
    y: clamp(region.y, 0, normalized.height - height),
    width,
    height,
  };
}

export function createRegionWithinBounds(
  start: Point,
  current: Point,
  bounds: Bounds,
): Region {
  const normalized = normalizedBounds(bounds);
  const startX = clamp(start.x, 0, normalized.width);
  const startY = clamp(start.y, 0, normalized.height);
  const currentX = clamp(current.x, 0, normalized.width);
  const currentY = clamp(current.y, 0, normalized.height);

  return {
    x: Math.min(startX, currentX),
    y: Math.min(startY, currentY),
    width: Math.abs(currentX - startX),
    height: Math.abs(currentY - startY),
  };
}

export function moveRegionWithinBounds(
  region: Region,
  delta: Point,
  bounds: Bounds,
): Region {
  return clampRegionToBounds(
    { ...region, x: region.x + delta.x, y: region.y + delta.y },
    bounds,
  );
}

export function resizeRegionWithinBounds(
  region: Region,
  handle: HandlePosition,
  delta: Point,
  bounds: Bounds,
  minimumSize = MIN_REGION_SIZE,
): Region {
  const normalized = normalizedBounds(bounds);
  const boundedRegion = clampRegionToBounds(region, normalized);
  const minimumWidth = Math.min(minimumSize, normalized.width);
  const minimumHeight = Math.min(minimumSize, normalized.height);
  let left = boundedRegion.x;
  let top = boundedRegion.y;
  let right = boundedRegion.x + boundedRegion.width;
  let bottom = boundedRegion.y + boundedRegion.height;

  if (handle.includes("w")) {
    left = clamp(left + delta.x, 0, right - minimumWidth);
  } else if (handle.includes("e")) {
    right = clamp(right + delta.x, left + minimumWidth, normalized.width);
  }

  if (handle.includes("n")) {
    top = clamp(top + delta.y, 0, bottom - minimumHeight);
  } else if (handle.includes("s")) {
    bottom = clamp(bottom + delta.y, top + minimumHeight, normalized.height);
  }

  return { x: left, y: top, width: right - left, height: bottom - top };
}

export function logicalScreenBounds(screen: VirtualScreenGeometry): Bounds {
  const scale = screen.scale_factor || 1;
  return {
    width: screen.total_width / scale,
    height: screen.total_height / scale,
  };
}

export function toPhysicalRegion(
  region: Region,
  screen: VirtualScreenGeometry,
): Region {
  const scale = screen.scale_factor || 1;
  const bounded = clampRegionToBounds(region, logicalScreenBounds(screen));
  const left = clamp(Math.round(bounded.x * scale), 0, screen.total_width);
  const top = clamp(Math.round(bounded.y * scale), 0, screen.total_height);
  const right = clamp(
    Math.round((bounded.x + bounded.width) * scale),
    left,
    screen.total_width,
  );
  const bottom = clamp(
    Math.round((bounded.y + bounded.height) * scale),
    top,
    screen.total_height,
  );

  return {
    x: left,
    y: top,
    width: right - left,
    height: bottom - top,
  };
}
