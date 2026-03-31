# COINjecture — Web Frontend

React 18 + TypeScript + Vite dashboard for the COINjecture testnet.

## Stack

| Layer | Technology |
|-------|-----------|
| Framework | React 18 |
| Language | TypeScript |
| Bundler | Vite |
| Styling | Tailwind CSS |
| Components | shadcn/ui |
| Icons | Lucide React |

## Routes

| Path | Page | Description |
|------|------|-------------|
| `/` | Index | Landing page — network overview |
| `/terminal` | Terminal | Live node log stream |
| `/api` | API | JSON-RPC API explorer |
| `/metrics` | Metrics | Network metrics dashboard |
| `/marketplace` | Marketplace | Problem marketplace |
| `/bounty-submit` | BountySubmit | Submit a new bounty |
| `/wallet` | Wallet | Browser-based testnet wallet |
| `/whitepaper` | Whitepaper | Protocol whitepaper viewer |

## Dev Setup

```bash
# Install dependencies
npm install

# Start dev server (http://localhost:5173)
npm run dev

# Build for production
npm run build

# Preview production build
npm run preview
```

## RPC Configuration

Copy `.env.example` to `.env.local` and set your RPC endpoint:

```bash
cp .env.example .env.local
```

| Variable | Default | Description |
|----------|---------|-------------|
| `VITE_RPC_URL` | `http://localhost:9933` | Comma-separated list of RPC node URLs |
| `VITE_METRICS_URL` | `http://localhost:9094` | Metrics endpoint (optional) |

For a local Docker testnet:
```bash
VITE_RPC_URL=http://localhost:9933
```

For multi-node production:
```bash
VITE_RPC_URL=https://rpc1.example.com,https://rpc2.example.com
```

## Testnet Notice

> **This is testnet software.** The wallet page stores Ed25519 private keys in browser localStorage. This is not secure. Do not use with real funds or on mainnet.

## Project Structure

```
src/
  components/      # Shared UI components (Navigation, etc.)
  pages/           # Route-level page components
  hooks/           # Custom React hooks
  lib/             # Utility functions
```
