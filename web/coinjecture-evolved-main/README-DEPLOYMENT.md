# COINjecture Frontend - Quick Deployment Guide

## Local Development

```bash
npm install
npm run dev
```

Frontend runs on `http://localhost:8080`

## Production Build

```bash
# Set environment variables
export VITE_RPC_URL=https://api.coinjecture.com:9933

# Build
npm run build

# Preview build locally
npm run preview
```

## AWS S3 + CloudFront Deployment

### Prerequisites

1. AWS CLI configured: `aws configure`
2. S3 bucket created: `aws s3 mb s3://coinjecture-frontend`
3. CloudFront distribution created (see DEPLOYMENT.md)

### Quick Deploy

```bash
# Set environment variables
export S3_BUCKET=coinjecture-frontend
export CLOUDFRONT_DIST_ID=d1234567890abc
export VITE_RPC_URL=https://api.coinjecture.com:9933

# Deploy
./deploy.sh
```

### Manual Deploy

```bash
# Build
npm run build

# Upload to S3
aws s3 sync dist/ s3://coinjecture-frontend --delete

# Invalidate CloudFront
aws cloudfront create-invalidation \
  --distribution-id YOUR_DIST_ID \
  --paths "/*"
```

## Environment Variables

Create `.env.production` for production builds:

```env
VITE_RPC_URL=https://api.coinjecture.com:9933
VITE_METRICS_URL=https://metrics.coinjecture.com:9094
```

## Features

- ✅ React 18 + TypeScript
- ✅ React Router for navigation
- ✅ TanStack Query for data fetching
- ✅ Tailwind CSS + shadcn/ui components
- ✅ RPC client integration
- ✅ Real-time marketplace data
- ✅ Responsive design
- ✅ Dark mode support
- ✅ Optimized for CloudFront CDN

## Project Structure

```
src/
├── components/       # React components
│   ├── ui/          # shadcn/ui components
│   └── ...          # Feature components
├── lib/             # Utilities and clients
│   └── rpc-client.ts # RPC client for blockchain API
├── pages/           # Page components
└── App.tsx          # Main app component
```

## Troubleshooting

**Build fails**: Check Node.js version (18+ required)

**RPC connection errors**: Verify `VITE_RPC_URL` is set correctly

**CORS errors**: Ensure RPC server allows requests from your domain

For detailed deployment instructions, see [DEPLOYMENT.md](./DEPLOYMENT.md)

