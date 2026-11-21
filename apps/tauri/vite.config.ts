import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;
const rootDir = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  resolve: {
    // Workspace package `imp-graph-view` must share the app's React — a nested
    // copy makes hooks read null (`Cannot read properties of null (reading 'useState')`).
    dedupe: ["react", "react-dom"],
    alias: {
      react: path.resolve(rootDir, "node_modules/react"),
      "react-dom": path.resolve(rootDir, "node_modules/react-dom"),
      "react/jsx-runtime": path.resolve(
        rootDir,
        "node_modules/react/jsx-runtime.js",
      ),
      "react/jsx-dev-runtime": path.resolve(
        rootDir,
        "node_modules/react/jsx-dev-runtime.js",
      ),
    },
  },
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
      ignored: ["**/src-tauri/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
