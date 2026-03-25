import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    port: 3001,
    host: true,
    allowedHosts: ['.trycloudflare.com'],
    proxy: {
      '/rpc': {
        target: 'http://127.0.0.1:9933',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/rpc/, '')
      },
      '/metrics': {
        target: 'http://127.0.0.1:9090',
        changeOrigin: true
      },
      '/marketplace': {
        target: 'http://127.0.0.1:8080',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/marketplace/, '')
      }
    }
  },
  build: {
    outDir: 'dist',
    // Disable sourcemaps in production to avoid exposing source code
    sourcemap: false,
  }
})
