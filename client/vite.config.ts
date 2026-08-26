import { defineConfig } from "vite";

export default defineConfig({
  server: {
    port: 5173,
    proxy: {
      // The dev server proxies the socket so the client always talks to a
      // same-origin /ws, exactly as it will in production.
      "/ws": { target: "ws://127.0.0.1:8080", ws: true },
      "/health": { target: "http://127.0.0.1:8080" },
    },
  },
  build: { target: "es2022", assetsInlineLimit: 0 },
});
