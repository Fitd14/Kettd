import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  root: 'src',
  resolve: {
    alias: {
      '@': '/src'
    }
  },
  build: {
    outDir: '../dist',
    assetsDir: 'assets',
    rollupOptions: {
      input: {
        main: './src/index.html',
        float: './src/float.html'
      }
    }
  }
})
