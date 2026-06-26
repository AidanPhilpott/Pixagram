import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed, known dev port and must not clear the terminal so
// Rust compiler output stays visible. See https://tauri.app/start/frontend/vite/
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: {
      // Don't watch the Rust source tree from Vite.
      ignored: ["**/src-tauri/**"],
    },
  },
  // Produce assets the Rust side can bundle; keep paths relative.
  build: {
    target: "esnext",
    minify: "esbuild",
    sourcemap: false,
  },
  // Only env vars prefixed with these are exposed to the frontend.
  envPrefix: ["VITE_", "TAURI_"],
});
