# Local Deployment Guide

## Quick Start

The frontend is now running locally! Here's what you need to know:

## Development Server

The dev server runs on **http://localhost:8080** by default.

### Start Development Server

```bash
cd web/coinjecture-evolved-main
npm run dev
```

### Stop Development Server

Press `Ctrl+C` in the terminal where the server is running.

## Environment Configuration

The `.env` file has been created with default local settings:

```env
VITE_RPC_URL=http://localhost:9933
VITE_METRICS_URL=http://localhost:9094
```

### Update RPC Endpoint

If your blockchain node is running on a different port or host, edit `.env`:

```bash
# Edit .env file
VITE_RPC_URL=http://your-node-ip:9933
```

Then restart the dev server.

## Available Scripts

- `npm run dev` - Start development server (hot reload)
- `npm run build` - Build for production
- `npm run build:production` - Build with production optimizations
- `npm run preview` - Preview production build locally
- `npm run lint` - Run ESLint

## Testing the Frontend

1. **Open Browser**: Navigate to `http://localhost:8080`
2. **Check Routes**:
   - `/` - Home page
   - `/marketplace` - Marketplace with live data
   - `/metrics` - Network metrics
   - `/api` - API documentation
   - `/terminal` - Terminal interface
   - `/whitepaper` - Whitepaper viewer

## Connecting to Blockchain Node

The frontend connects to your blockchain node via RPC. Make sure:

1. **Node is Running**: Your COINjecture node should be running on port 9933
2. **CORS Enabled**: If node is on different origin, enable CORS:
   ```rust
   // In your RPC server configuration
   .wrap(cors().allow_any_origin())
   ```
3. **Check Connection**: Open browser console and check for RPC errors

## Troubleshooting

### Port Already in Use

If port 8080 is taken, Vite will automatically use the next available port.

### RPC Connection Errors

1. Verify node is running: `curl http://localhost:9933`
2. Check `.env` file has correct URL
3. Check browser console for CORS errors
4. Verify node RPC is enabled

### Build Errors

```bash
# Clear cache and rebuild
rm -rf node_modules dist
npm install
npm run build
```

### Hot Reload Not Working

- Check file watchers aren't exhausted (macOS: `ulimit -n 4096`)
- Restart the dev server

## Production Build Preview

To test the production build locally:

```bash
npm run build:production
npm run preview
```

This serves the optimized build on a local server (usually port 4173).

## Next Steps

Once local deployment is working:

1. ✅ Test all routes
2. ✅ Verify RPC connectivity
3. ✅ Test marketplace data loading
4. ✅ Check mobile responsiveness
5. ✅ Ready for CloudFront deployment

## Development Tips

- **Hot Reload**: Changes to React components auto-reload
- **TypeScript**: Type errors shown in terminal
- **Console**: Check browser DevTools for errors
- **Network Tab**: Monitor RPC requests

---

**Status**: ✅ Running locally on http://localhost:8080

