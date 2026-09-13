import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Vite only serves the webview bundle; all IPC goes through Tauri's `invoke`.
export default defineConfig({
  plugins: [react()],
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  clearScreen: false,
  build: {
    target: ["es2022", "chrome110", "safari16"],
    sourcemap: false,
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    port: 5174,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
  },
});
