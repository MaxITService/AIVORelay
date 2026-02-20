# AivoRelay Microsoft Store Build & Differences

This file documents the exact technical differences, file paths, and code rows that differ between the `main` branch and the `Microsoft-store` branch.

Because the Microsoft Store has strict policies (e.g., no self-updating, sandboxing, varying hardware), this branch maintains specific configurations that **must not be overwritten** when merging updates from `main`.

---

## 1. GitHub Action Workflow
**File:** `.github/workflows/microsoft-store-release.yml`

This workflow is used strictly for drafting Microsoft Store releases. We turn off standard binary signing because the Microsoft Store ingest process signs the MSI/MSIX for us.
**Code Changes:**
```diff
@@ -58,7 +58,7 @@ jobs:
       platform: ${{ matrix.platform }}
       target: ${{ matrix.target }}
       build-args: ${{ matrix.args }}
-      sign-binaries: true
+      sign-binaries: false
       asset-prefix: "aivorelay-store"
```

---

## 2. Core Agent & Developer Documentation
**File:** `AGENTS.md`

We append a critical warning at the very top of the Developer/Agent guide so AI assistants and developers know they are interacting with the Store Edition and should not commit changes directly or attempt to build standard auto-updaters.
**Code Changes:**
```diff
@@ -1,6 +1,11 @@
 # Fork Agents Guide
 
+> **CRITICAL: WE ARE ON THE `Microsoft-store` BRANCH.**
+> This branch is specifically for the Microsoft Store release.
+> **AGENT RULE:** Always refer to this version as the **Microsoft Store Edition**.
+> All updates must be compliant with Microsoft Store policies (e.g., no self-updating, sandboxed file access in mind (MSIX packaged, this will be handled atomatically later)). Warn the user in case something is not compatible with the Microsoft Store. 
+
 > **Agent rule:** all debugging/build verification is done by the user
```

---

## 3. CPU AVX Instruction Limitations
**Files:** `src-tauri/.cargo/config.toml`, `src-tauri/cmake/force_ggml_avx2.cmake`

By default, upstream `whisper.cpp`/`ggml` can auto-detect AVX-512 on MSVC and emit `/arch:AVX512` even when Rust is built with `+avx2`. Since Microsoft Store distributes one binary to many CPU generations, we force both layers:
1. Rust code generation stays on `+avx2`
2. `whisper.cpp`/`ggml` CMake is forced to AVX2 and AVX512 is disabled

This prevents Store Edition crashes on CPUs without AVX-512 support.

*(Note: this Store-specific setup does not exist on `main` branch.)*
**Code Changes:**
```toml
[build]
rustflags = ["-C", "target-feature=+avx2"]

[env]
CMAKE_PROJECT_INCLUDE_BEFORE = { value = "../cmake/force_ggml_avx2.cmake", relative = true, force = true }
```

```cmake
# src-tauri/cmake/force_ggml_avx2.cmake
set(GGML_NATIVE OFF CACHE BOOL "" FORCE)
set(GGML_AVX2 ON CACHE BOOL "" FORCE)
set(GGML_AVX512 OFF CACHE BOOL "" FORCE)
```

---

## 4. Tauri Configuration & Updaters
**File:** `src-tauri/tauri.conf.json`

Because applications distributed via the Microsoft Store are required to use the Microsoft Store's native update delivery system, we completely strip out Tauri's built-in update functionality. We also remove signing certificates and change the window title.
**Code Changes:**
```diff
@@ -16,7 +16,7 @@
     "windows": [
       {
         "label": "main",
-        "title": "AivoRelay",
+        "title": "AivoRelay (Store Edition)",
         "width": 680,
@@ -42,7 +42,7 @@
     "publisher": "MaxITService",
     "copyright": "Copyright © 2026 Maxim Fomin",
     "shortDescription": "AivoRelay - AI Voice Relay",
-    "createUpdaterArtifacts": true,
+    "createUpdaterArtifacts": false,
     "targets": "msi",
@@ -72,8 +72,6 @@
       }
     },
     "windows": {
-      "certificateThumbprint": "C1F83B324662E8D224282F9D587B030426346A77",
-      "digestAlgorithm": "sha256",
       "wix": {
         "template": "./windows/wix/main.wxs"
       }
@@ -82,9 +80,7 @@
   "plugins": {
     "updater": {
       "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEM4NjVDMjVCMEY3REY4OTYKUldTVytIMFBXOEpseUk1NCtIM0ROTklHdS9MVUNyNVZTM3c5ZFoxdXRRdWE5cjVUaWRpbUhTUHoK",
-      "endpoints": [
-        "https://github.com/MaxITService/AIVORelay/releases/latest/download/latest.json"
-      ]
+      "endpoints": []
     }
```

---

## 5. UI: Application Footer
**File:** `src/components/footer/Footer.tsx`

Since the actual `tauri.conf.json` updater endpoints are removed, we must also remove the frontend React components that allow the user to click "Check for Updates" to prevent application logic crashes.
**Code Changes:**
```diff
@@ -4,7 +4,6 @@
 import { getVersion } from "@tauri-apps/api/app";
 
 import ModelSelector from "../model-selector";
-import UpdateChecker from "../update-checker";
 import VramMeter from "./VramMeter";
 
@@ -35,10 +34,8 @@
           <VramMeter refreshNonce={vramRefreshNonce} />
         </div>
 
-        {/* Update Status */}
+        {/* Version info */}
         <div className="flex items-center gap-2">
-          <UpdateChecker />
-          <span className="text-[#333333]">•</span>
           {/* eslint-disable-next-line i18next/no-literal-string */}
           <span className="font-medium">v{version}</span>
         </div>
```

---

## 6. UI: Settings Menu
**File:** `src/components/settings/debug/DebugSettings.tsx`

Similarly, the settings page contains a toggle element to automatically check for updates. We remove this React component entirely from the Store branch.
**Code Changes:**
```diff
@@ -16,7 +16,6 @@
 import { RecordingRetentionPeriodSelector } from "../RecordingRetentionPeriod";
 import { ClamshellMicrophoneSelector } from "../ClamshellMicrophoneSelector";
 import { HandyShortcut } from "../HandyShortcut";
-import { UpdateChecksToggle } from "../UpdateChecksToggle";
 import { ToggleSwitch } from "../../ui/ToggleSwitch";
 import { ConfirmationModal } from "../../ui/ConfirmationModal";
 
@@ -54,7 +53,6 @@
       <SettingsGroup title={t("settings.debug.title")}>
         <LogDirectory grouped={true} />
         <LogLevelSelector grouped={true} />
-        <UpdateChecksToggle descriptionMode="tooltip" grouped={true} />
         <SoundPicker
           label={t("settings.debug.soundTheme.label")}
```
