import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The dev server proxies API and health calls to the Rust backend so the SPA
// runs same-origin in development, matching production (Axum serves web/dist).
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      // The backend's CSRF check requires the `Origin` host to equal `Host`. Going
      // through the dev proxy makes them differ (Origin = :5173, Host = :8080),
      // which would 403 every state-changing request (login/POST/...). Strip the
      // `Origin` header on proxied API calls — the backend allows requests with no
      // Origin — so dev login works. Dev-only; production is genuinely same-origin.
      "/api": {
        target: "http://localhost:8080",
        changeOrigin: true,
        configure: (proxy) => {
          proxy.on("proxyReq", (proxyReq) => proxyReq.removeHeader("origin"));
        },
      },
      "/health": "http://localhost:8080",
    },
  },
  build: {
    outDir: "dist",
  },
});
