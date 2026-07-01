import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  root: ".",
  base: "./",
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    port: 1420,
    strictPort: true,
    host: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  optimizeDeps: {
    // 预构建大依赖：xterm 体积大、addon-fit 依赖 xterm，zustand 经常被多文件 import
    // 避免 Vite 启动时按需发现依赖导致首次加载慢
    include: ["@xterm/xterm", "@xterm/addon-fit", "zustand"],
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
});
