import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The dev server proxies API and health calls to the Rust backend so the SPA
// runs same-origin in development, matching production (Axum serves web/dist).
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      "/api": "http://localhost:8080",
      "/health": "http://localhost:8080",
    },
  },
  build: {
    outDir: "dist",
  },
});
