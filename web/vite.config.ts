import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The API is same-origin in production (axum serves this bundle). In dev the
// server runs on :8080 and Vite proxies /api and /healthz to it.
export default defineConfig({
  plugins: [react()],
  build: {
    target: "es2022",
    sourcemap: false,
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    proxy: {
      "/api": { target: "http://127.0.0.1:8080", ws: true },
      "/healthz": "http://127.0.0.1:8080",
    },
  },
});
