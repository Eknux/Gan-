import { defineConfig } from "vite";

// Tauri 2: dev sunucusu 1420 portunda (tauri.conf.json devUrl ile eşleşmeli),
// 0.0.0.0'a bağlanır ki Android emülatörü ana makinedeki dev sunucusuna erişsin.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "0.0.0.0",
    hmr: { protocol: "ws", host: "localhost", port: 1421 },
  },
  build: {
    target: "es2021",
    outDir: "dist",
    emptyOutDir: true,
    sourcemap: false,
  },
});
