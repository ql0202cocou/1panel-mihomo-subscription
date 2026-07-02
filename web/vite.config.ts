import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The dev server proxies API and health calls to the Rust backend so the SPA
// runs same-origin in development, matching production (Axum serves web/dist).
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      // Keep the browser's Host/Origin pair when proxying. The backend rejects
      // missing or cross-origin state-changing requests; preserving both headers
      // makes local dev match the same-origin production shape.
      "/api": {
        target: "http://localhost:8080",
      },
      "/health": "http://localhost:8080",
    },
  },
  build: {
    outDir: "dist",
  },
});
