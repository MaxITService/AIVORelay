import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { mkdirSync, rmSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";

const host = process.env.TAURI_DEV_HOST;

// CI release builds keep sourcemaps to decode the frontend stack traces in
// aivorelay.log. "hidden" leaves no sourceMappingURL comment, and the maps go
// outside `dist`, which Tauri embeds whole into the app.
const withSourcemaps = process.env.AIVORELAY_SOURCEMAPS === "1";
const sourcemapDir = resolve(__dirname, "dist-sourcemaps");

const keepSourcemapsOutsideDist = (): Plugin => ({
  name: "keep-sourcemaps-outside-dist",
  apply: "build",
  buildStart() {
    rmSync(sourcemapDir, { recursive: true, force: true });
  },
  generateBundle(_options, bundle) {
    for (const [fileName, file] of Object.entries(bundle)) {
      if (file.type !== "asset" || !fileName.endsWith(".map")) continue;
      const target = resolve(sourcemapDir, fileName);
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, file.source);
      delete bundle[fileName];
    }
  },
});

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [
    react(),
    tailwindcss(),
    withSourcemaps && keepSourcemapsOutsideDist(),
  ],

  // Path aliases
  resolve: {
    alias: {
      "@": resolve(__dirname, "./src"),
      "@/bindings": resolve(__dirname, "./src/bindings.ts"),
    },
  },

  // Multiple entry points for main app, overlay, and region capture
  build: {
    sourcemap: withSourcemaps ? ("hidden" as const) : false,
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        overlay: resolve(__dirname, "src/overlay/index.html"),
        "region-capture": resolve(__dirname, "src/region-capture/index.html"),
        "command-confirm": resolve(__dirname, "src/command-confirm/index.html"),
        "voice-activation-button": resolve(
          __dirname,
          "src/voice-activation-button/index.html",
        ),
        "soniox-live-preview": resolve(
          __dirname,
          "src/soniox-live-preview/index.html",
        ),
        "tts-overlay": resolve(__dirname, "src/tts-overlay/index.html"),
      },
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
