import { defineConfig } from "vite";

export default defineConfig({
  root: ".",
  publicDir: "public",
  build: {
    outDir: "dist",
    target: "esnext",
    sourcemap: true,
    // One JS file — Babylon's 400+ chunk split + SPA HTML fallback was
    // poisoning missing chunk loads and leaving the game stuck on "Initializing".
    cssCodeSplit: false,
    modulePreload: false,
    rollupOptions: {
      output: {
        inlineDynamicImports: true,
        entryFileNames: "assets/game.js",
        assetFileNames: "assets/[name]-[hash][extname]",
      },
    },
  },
  server: {
    port: 5173,
    strictPort: false,
  },
  optimizeDeps: {
    exclude: ["@babylonjs/core"],
  },
});
