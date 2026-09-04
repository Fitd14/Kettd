import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'

// Kettd v3 · 前端基座（M0）
// base './' —— 后续 Tauri 以自定义协议从 dist 加载时，产物内引用走相对路径
// server 固定端口，便于之后 tauri devPath 指向 http://localhost:5173
export default defineConfig({
  plugins: [react(), tailwindcss()],
  base: './',
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  server: {
    port: 5173,
    strictPort: true,
    host: true,
  },
  build: {
    outDir: 'dist',
    target: 'es2022',
  },
})
