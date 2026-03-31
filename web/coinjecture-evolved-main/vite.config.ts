import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react-swc";
import path from "path";

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => {
  // Load env file based on `mode` in the current working directory.
  const env = loadEnv(mode, process.cwd(), '');
  
  // Get the first RPC URL from VITE_RPC_URL (comma-separated)
  const rpcUrl = (env.VITE_RPC_URL || 'http://localhost:9933').split(',')[0].trim();
  
  return {
    server: {
      host: "::",
      port: 8080,
      proxy: {
        // Proxy RPC requests to avoid CORS issues in development
        '/api/rpc': {
          target: rpcUrl,
          changeOrigin: true,
          rewrite: (path) => {
            // Remove /api/rpc prefix and send to root of target
            const rewritten = path.replace(/^\/api\/rpc/, '');
            return rewritten || '/';
          },
          configure: (proxy, _options) => {
            proxy.on('error', (err, req, res) => {
              console.error('Proxy error:', err);
            });
            proxy.on('proxyReq', (proxyReq, req, res) => {
              console.log('Proxying request to:', rpcUrl);
            });
            proxy.on('proxyRes', (proxyRes, req, res) => {
              // Add CORS headers to response
              proxyRes.headers['Access-Control-Allow-Origin'] = '*';
              proxyRes.headers['Access-Control-Allow-Methods'] = 'POST, GET, OPTIONS';
              proxyRes.headers['Access-Control-Allow-Headers'] = 'Content-Type';
            });
          },
        },
      },
    },
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    sourcemap: false,
    minify: mode === 'production' ? 'terser' : false,
    terserOptions: mode === 'production' ? {
      compress: {
        drop_console: true,
        drop_debugger: true,
      },
    } : undefined,
    rollupOptions: {
      output: {
        // Add timestamp to chunk file names to force cache-busting
        entryFileNames: `assets/[name]-[hash]-v1.0.1.js`,
        chunkFileNames: `assets/[name]-[hash]-v1.0.1.js`,
        assetFileNames: `assets/[name]-[hash]-v1.0.1.[ext]`,
        manualChunks: {
          'react-vendor': ['react', 'react-dom', 'react-router-dom'],
          'query-vendor': ['@tanstack/react-query'],
          'chart-vendor': ['recharts'],
        },
      },
    },
    chunkSizeWarningLimit: 1000,
  },
    // Environment variables are automatically available via import.meta.env
    // No need to define them here - Vite handles VITE_* prefixed variables automatically
  };
});
