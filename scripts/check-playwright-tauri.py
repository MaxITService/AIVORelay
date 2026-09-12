#!/usr/bin/env python3
"""Verify that Playwright can attach to the running AivoRelay Tauri window."""

from __future__ import annotations

import argparse
import asyncio
import json
import sys
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Attach to AivoRelay over WebView2 CDP and verify the main page.",
    )
    parser.add_argument("--port", type=int, default=9333)
    parser.add_argument("--timeout-ms", type=int, default=10_000)
    parser.add_argument(
        "--screenshot",
        type=Path,
        help="Optional path for a screenshot of the attached main window.",
    )
    return parser.parse_args()


async def check(args: argparse.Namespace) -> dict[str, object]:
    try:
        from playwright.async_api import async_playwright
    except ImportError as error:
        raise RuntimeError(
            "Python Playwright is not installed. Run: "
            "python -m pip install --upgrade playwright",
        ) from error

    endpoint = f"http://127.0.0.1:{args.port}"
    async with async_playwright() as playwright:
        browser = await playwright.chromium.connect_over_cdp(
            endpoint,
            timeout=args.timeout_ms,
        )
        try:
            pages = [page for context in browser.contexts for page in context.pages]
            targets = [
                {"title": await page.title(), "url": page.url}
                for page in pages
            ]
            main_page = None
            for page in pages:
                if (
                    await page.title() == "AivoRelay"
                    and not page.url.endswith("/src/overlay/index.html")
                ):
                    main_page = page
                    break
            if main_page is None:
                raise RuntimeError(
                    "CDP is reachable, but the AivoRelay main window was not found. "
                    f"Targets: {targets}",
                )

            root = main_page.locator("#root")
            await root.wait_for(state="visible", timeout=args.timeout_ms)
            visible_text = (await root.inner_text()).strip()
            if not visible_text:
                raise RuntimeError("The AivoRelay root is visible but contains no text.")

            screenshot_path = None
            if args.screenshot:
                screenshot_path = args.screenshot.resolve()
                screenshot_path.parent.mkdir(parents=True, exist_ok=True)
                await main_page.screenshot(path=str(screenshot_path))

            return {
                "ok": True,
                "endpoint": endpoint,
                "browserVersion": browser.version,
                "targetCount": len(targets),
                "targets": targets,
                "mainWindow": {
                    "title": await main_page.title(),
                    "url": main_page.url,
                    "rootVisible": await root.is_visible(),
                    "visibleTextCharacters": len(visible_text),
                },
                "screenshot": str(screenshot_path) if screenshot_path else None,
            }
        finally:
            await browser.close()


def main() -> int:
    args = parse_args()
    if not 1 <= args.port <= 65_535:
        print("error: --port must be between 1 and 65535", file=sys.stderr)
        return 2
    if args.timeout_ms <= 0:
        print("error: --timeout-ms must be positive", file=sys.stderr)
        return 2

    try:
        result = asyncio.run(check(args))
    except Exception as error:
        print(json.dumps({"ok": False, "error": str(error)}, indent=2))
        return 1

    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
