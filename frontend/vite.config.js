import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Relative base so the bundle works when served by Tauri from a file/asset URL.
export default defineConfig({
  plugins: [vue()],
  base: "./",
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: "esnext",
  },
  // Tauri dev server defaults (used by `npm run dev` + `tauri dev`).
  server: {
    port: 5173,
    strictPort: true,
  },
});
