/// <reference types="vitest" />
import { fileURLToPath, URL } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig(({ mode }) => ({
  plugins: [react()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: false,
    hmr: { protocol: "ws", host: "localhost", port: 1421 },
  },
  envPrefix: ["VITE_", "TAURI_"],
  // Strip console + debugger calls from production bundles. Tauri-specific
  // diagnostics still flow through the Rust side, and any rogue console.log
  // would leak in dev builds the user sees in tauri devtools.
  esbuild: mode === "production" ? { drop: ["console", "debugger"] } : {},
  build: {
    target: "chrome120",
    minify: "esbuild",
    sourcemap: true,
    outDir: "dist",
    rollupOptions: {
      output: {
        manualChunks: (id) => {
          if (id.includes("node_modules")) {
            if (
              id.includes("/react/") ||
              id.includes("/react-dom/") ||
              id.includes("/react-error-boundary/") ||
              id.includes("/framer-motion/") ||
              id.includes("/zustand/")
            ) {
              return "vendor";
            }
            if (id.includes("/@tauri-apps/")) {
              return "tauri";
            }
          }
          return undefined;
        },
      },
    },
  },
  test: {
    environment: "happy-dom",
    globals: true,
    include: ["src/**/*.test.{ts,tsx}"],
  },
}));
