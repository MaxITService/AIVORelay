#!/usr/bin/env python3
"""Exercise the TTS voice gallery in a running AivoRelay Tauri dev build."""

from __future__ import annotations

import argparse
import asyncio
import json
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = (
    REPO_ROOT
    / "src"
    / "components"
    / "settings"
    / "text-to-speech"
    / "ttsVoiceGalleryManifest.json"
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Attach to AivoRelay over WebView2 CDP and exercise the TTS voice "
            "gallery on both TTS pages. The original TTS settings are restored."
        ),
    )
    parser.add_argument("--port", type=int, default=9333)
    parser.add_argument("--timeout-ms", type=int, default=15_000)
    return parser.parse_args()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


async def invoke(page: Any, command: str, args: dict[str, Any] | None = None) -> Any:
    return await page.evaluate(
        """async ({ command, args }) => {
            const invoke = window.__TAURI_INTERNALS__?.invoke;
            if (typeof invoke !== "function") {
                throw new Error("window.__TAURI_INTERNALS__.invoke is unavailable");
            }
            return await invoke(command, args ?? {});
        }""",
        {"command": command, "args": args or {}},
    )


async def find_main_page(browser: Any) -> tuple[Any, list[dict[str, str]]]:
    pages = [page for context in browser.contexts for page in context.pages]
    targets = [{"title": await page.title(), "url": page.url} for page in pages]
    for page in pages:
        if (
            await page.title() == "AivoRelay"
            and not page.url.endswith("/src/overlay/index.html")
        ):
            return page, targets
    raise RuntimeError(
        "CDP is reachable, but the AivoRelay main window was not found. "
        f"Targets: {targets}",
    )


async def check_gallery_page(
    page: Any,
    section_id: str,
    voice_ids: list[str],
    timeout_ms: int,
) -> dict[str, object]:
    await page.get_by_test_id(f"sidebar-section-{section_id}").click()
    gallery = page.locator("#tts-voice-gallery")
    await gallery.wait_for(state="visible", timeout=timeout_ms)

    toggle = page.get_by_test_id("tts-voice-gallery-toggle")
    require(
        await toggle.get_attribute("aria-expanded") == "false",
        f"Gallery is not collapsed by default on {section_id}",
    )
    await toggle.click()
    await page.get_by_test_id(f"tts-voice-card-{voice_ids[0]}").wait_for(
        state="visible",
        timeout=timeout_ms,
    )

    cards = gallery.locator('[data-testid^="tts-voice-card-"]')
    audio_wrappers = gallery.locator('[data-testid^="tts-voice-audio-"]')
    apply_buttons = gallery.locator('[data-testid^="tts-voice-apply-"]')
    require(await cards.count() == len(voice_ids), "Unexpected gallery card count")
    require(
        await audio_wrappers.count() == len(voice_ids),
        "Unexpected gallery audio preview count",
    )
    require(
        await apply_buttons.count() == len(voice_ids),
        "Unexpected gallery Apply button count",
    )

    rendered_ids = await cards.evaluate_all(
        """elements => elements.map(element =>
            element.getAttribute("data-testid").replace("tts-voice-card-", "")
        )""",
    )
    require(rendered_ids == voice_ids, "Gallery card order differs from the manifest")

    await page.wait_for_function(
        """expected => {
            const audio = [...document.querySelectorAll(
                "#tts-voice-gallery [data-testid^='tts-voice-audio-'] audio"
            )];
            return audio.length === expected && audio.every(item => item.readyState >= 1);
        }""",
        arg=len(voice_ids),
        timeout=timeout_ms,
    )

    sources = await gallery.locator("audio").evaluate_all(
        "elements => elements.map(element => element.currentSrc || element.src)",
    )
    require(
        all(source.endswith(".opus") for source in sources),
        "A gallery preview does not use an .opus asset",
    )

    return {
        "section": section_id,
        "cards": len(rendered_ids),
        "opusPreviewsReady": len(sources),
        "collapsedByDefault": True,
    }


def active_scope_config(tts: dict[str, Any], scope_name: str) -> dict[str, Any] | None:
    scope = tts.get(scope_name) or {}
    active_key = scope.get("active_model_key")
    for model in scope.get("models") or []:
        if model.get("model_key") == active_key:
            return model.get("config")
    return None


async def check(args: argparse.Namespace) -> dict[str, object]:
    try:
        from playwright.async_api import async_playwright
    except ImportError as error:
        raise RuntimeError(
            "Python Playwright is not installed. Run: "
            "python -m pip install --upgrade playwright",
        ) from error

    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    voices = manifest["voices"]
    voice_ids = [entry["id"] for entry in voices]
    endpoint = f"http://127.0.0.1:{args.port}"

    async with async_playwright() as playwright:
        browser = await playwright.chromium.connect_over_cdp(
            endpoint,
            timeout=args.timeout_ms,
        )
        page = None
        original_tts = None
        original_section_test_id = None
        settings_restored = False
        console_errors: list[str] = []
        page_errors: list[str] = []
        try:
            page, targets = await find_main_page(browser)
            await page.locator("#root").wait_for(
                state="visible",
                timeout=args.timeout_ms,
            )
            page.on(
                "console",
                lambda message: console_errors.append(message.text)
                if message.type == "error"
                else None,
            )
            page.on("pageerror", lambda error: page_errors.append(str(error)))

            active_sidebar_item = page.locator(
                '[data-testid^="sidebar-section-"][aria-current="page"]',
            )
            if await active_sidebar_item.count():
                original_section_test_id = await active_sidebar_item.first.get_attribute(
                    "data-testid",
                )

            app_settings = await invoke(page, "get_app_settings")
            original_tts = app_settings["tts"]
            original_output = {
                "output_format": original_tts.get("output_format"),
                "mp3_bitrate_kbps": original_tts.get("mp3_bitrate_kbps"),
                "opus_bitrate_kbps": original_tts.get("opus_bitrate_kbps"),
            }

            pages = []
            for section_id in ("textToSpeech", "ttsFiles"):
                pages.append(
                    await check_gallery_page(
                        page,
                        section_id,
                        voice_ids,
                        args.timeout_ms,
                    ),
                )

            original_config = active_scope_config(original_tts, "file_synthesis") or {}
            chosen = next(
                entry
                for entry in voices
                if (
                    entry["provider"],
                    entry["model"],
                    entry["voice"],
                )
                != (
                    original_config.get("provider"),
                    original_config.get("model"),
                    original_config.get("voice"),
                )
            )
            await page.get_by_test_id(f"tts-voice-apply-{chosen['id']}").click()

            async def gallery_settings_applied() -> bool:
                current = (await invoke(page, "get_app_settings"))["tts"]
                config = active_scope_config(current, "file_synthesis") or {}
                return (
                    config.get("provider") == chosen["provider"]
                    and config.get("model") == chosen["model"]
                    and config.get("voice") == chosen["voice"]
                )

            deadline = asyncio.get_running_loop().time() + args.timeout_ms / 1000
            while not await gallery_settings_applied():
                if asyncio.get_running_loop().time() >= deadline:
                    raise RuntimeError("Gallery settings were not applied before timeout")
                await asyncio.sleep(0.1)

            applied_tts = (await invoke(page, "get_app_settings"))["tts"]
            applied_output = {
                "output_format": applied_tts.get("output_format"),
                "mp3_bitrate_kbps": applied_tts.get("mp3_bitrate_kbps"),
                "opus_bitrate_kbps": applied_tts.get("opus_bitrate_kbps"),
            }
            require(
                applied_output == original_output,
                "Applying a gallery voice changed output format or bitrate settings",
            )
            require(not page_errors, f"Page errors: {page_errors}")
            require(not console_errors, f"Console errors: {console_errors}")

            return {
                "ok": True,
                "endpoint": endpoint,
                "browserVersion": browser.version,
                "manifest": {
                    "schemaVersion": manifest["schemaVersion"],
                    "voices": len(voices),
                    "providers": len(manifest["providers"]),
                },
                "pages": pages,
                "apply": {
                    "voiceId": chosen["id"],
                    "provider": chosen["provider"],
                    "model": chosen["model"],
                    "voice": chosen["voice"],
                    "outputSettingsPreserved": True,
                },
                "targets": targets,
            }
        finally:
            if page is not None and original_tts is not None:
                try:
                    await invoke(
                        page,
                        "update_tts_settings",
                        {
                            "settings": original_tts,
                            "scope": "file",
                            "changedField": "__playwright_restore__",
                        },
                    )
                    settings_restored = True
                finally:
                    if original_section_test_id:
                        locator = page.get_by_test_id(original_section_test_id)
                        if await locator.count():
                            await locator.click()
            await browser.close()
            if original_tts is not None and not settings_restored:
                raise RuntimeError("The E2E check could not restore the original TTS settings")


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
