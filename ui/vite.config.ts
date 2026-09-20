import { defineConfig } from "vite";

// Tauri expects a fixed port and no clearing of the screen.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: "es2021",
    minify: "esbuild",
    sourcemap: false,
    outDir: "dist",
  },
});
