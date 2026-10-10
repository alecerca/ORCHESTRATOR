import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri injects TAURI_DEV_HOST when developing on a physical device.
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  // Tauri expects a fixed port and fails if it is not available.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: {
      // Never watch the Rust side from Vite.
      ignored: ["**/src-tauri/**"],
    },
  },
  // Monaco ships its workers as ES modules.
  worker: {
    format: "es",
  },
  build: {
    target: "es2021",
    sourcemap: true,
    emptyOutDir: true,
  },
});
