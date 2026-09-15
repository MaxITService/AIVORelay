import { describe, expect, test } from "bun:test";

import {
  createRegionWithinBounds,
  moveRegionWithinBounds,
  resizeRegionWithinBounds,
  toPhysicalRegion,
  type Bounds,
  type HandlePosition,
} from "./geometry";

const bounds: Bounds = { width: 800, height: 600 };

describe("region capture geometry", () => {
  test("creation remains inside every canvas edge", () => {
    expect(
      createRegionWithinBounds({ x: 100, y: 100 }, { x: -20, y: -30 }, bounds),
    ).toEqual({ x: 0, y: 0, width: 100, height: 100 });
    expect(
      createRegionWithinBounds({ x: 700, y: 500 }, { x: 900, y: 700 }, bounds),
    ).toEqual({ x: 700, y: 500, width: 100, height: 100 });
  });

  test("moving clamps all edges and corners", () => {
    const region = { x: 100, y: 100, width: 300, height: 200 };
    expect(moveRegionWithinBounds(region, { x: -240, y: -180 }, bounds)).toEqual({
      x: 0,
      y: 0,
      width: 300,
      height: 200,
    });
    expect(moveRegionWithinBounds(region, { x: 700, y: 600 }, bounds)).toEqual({
      x: 500,
      y: 400,
      width: 300,
      height: 200,
    });
  });

  test("resizing clamps every handle to the canvas", () => {
    const region = { x: 100, y: 100, width: 300, height: 200 };
    const cases: Array<[HandlePosition, { x: number; y: number }, object]> = [
      ["nw", { x: -200, y: -200 }, { x: 0, y: 0, width: 400, height: 300 }],
      ["n", { x: 0, y: -200 }, { x: 100, y: 0, width: 300, height: 300 }],
      ["ne", { x: 600, y: -200 }, { x: 100, y: 0, width: 700, height: 300 }],
      ["w", { x: -200, y: 0 }, { x: 0, y: 100, width: 400, height: 200 }],
      ["e", { x: 600, y: 0 }, { x: 100, y: 100, width: 700, height: 200 }],
      ["sw", { x: -200, y: 500 }, { x: 0, y: 100, width: 400, height: 500 }],
      ["s", { x: 0, y: 500 }, { x: 100, y: 100, width: 300, height: 500 }],
      ["se", { x: 600, y: 500 }, { x: 100, y: 100, width: 700, height: 500 }],
    ];

    for (const [handle, delta, expected] of cases) {
      expect(resizeRegionWithinBounds(region, handle, delta, bounds)).toEqual(
        expected,
      );
    }
  });

  test("resizing preserves the minimum region size", () => {
    const region = { x: 100, y: 100, width: 300, height: 200 };
    expect(
      resizeRegionWithinBounds(region, "nw", { x: 500, y: 500 }, bounds),
    ).toEqual({ x: 390, y: 290, width: 10, height: 10 });
  });

  test("a full-screen region cannot be moved outside the canvas", () => {
    expect(
      moveRegionWithinBounds(
        { x: 0, y: 0, width: 800, height: 600 },
        { x: -100, y: 100 },
        bounds,
      ),
    ).toEqual({ x: 0, y: 0, width: 800, height: 600 });
  });

  test("fractional scaling rounds edges without overflowing physical bounds", () => {
    expect(
      toPhysicalRegion(
        { x: 0.4, y: 0.4, width: 99.6, height: 79.6 },
        { total_width: 125, total_height: 100, scale_factor: 1.25 },
      ),
    ).toEqual({ x: 1, y: 1, width: 124, height: 99 });
  });
});
