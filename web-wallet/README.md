# COINjecture Network B - Web Wallet & Explorer

Modern browser-based wallet and testnet explorer for COINjecture Network B.

## Features

### 🔐 Wallet
- Client-side Ed25519 keypair generation
- Secure key storage in browser
- View account balances
- Send transactions
- Transaction history

### 🔍 Explorer
- View latest blocks
- Browse block history
- View block details and transactions
- Real-time chain statistics
- Network status monitoring

### 📊 Metrics Dashboard
- Dimensional pool balances (D1, D2, D3)
- Satoshi constants (η, λ)
- Unit circle constraint validation
- Real-time Prometheus metrics
- Interactive charts

## Installation

```bash
cd web-wallet
npm install
```

## Development

```bash
npm run dev
```

Opens on `http://localhost:3001`

The dev server proxies to:
- `/rpc` → `http://localhost:8545` (blockchain RPC)
- `/metrics` → `http://localhost:9094` (Prometheus metrics)

## Build for Production

```bash
npm run build
```

Output in `dist/` directory.

## Deployment

### Option 1: Static Hosting (Netlify, Vercel, Cloudflare Pages)

1. Build the project:
   ```bash
   npm run build
   ```

2. Deploy the `dist/` folder

3. Configure environment variable for RPC endpoint:
   - Set `VITE_RPC_URL` to your node's public RPC endpoint

### Option 2: Docker + Nginx

See `Dockerfile` and `nginx.conf` for containerized deployment.

### Option 3: CDN Distribution

The built static files can be served from any CDN.

## Configuration

### Environment Variables

Create `.env` file:

```env
VITE_RPC_URL=http://your-node-ip:8545
VITE_METRICS_URL=http://your-node-ip:9094
```

### Network Configuration

Edit `vite.config.ts` to change proxy targets for local development.

## Security Notes

⚠️ **TESTNET ONLY** ⚠️

- Private keys are stored in browser localStorage
- This is NOT secure for mainnet or real funds
- For production, implement hardware wallet support or browser extension

## Architecture

- **Frontend**: React 18 + TypeScript
- **Build Tool**: Vite
- **State Management**: React Query
- **Crypto**: @noble/ed25519 + @noble/hashes
- **Charts**: Recharts
- **Icons**: Lucide React

## API Integration

### RPC Methods

- `get_block(height)` - Get block by height
- `get_latest_block()` - Get latest block
- `get_account_balance(address)` - Get account balance
- `get_account_info(address)` - Get full account info
- `submit_transaction(tx_hex)` - Submit signed transaction
- `get_chain_info()` - Get chain statistics

### Prometheus Metrics

- `coinject_pool_balance` - Dimensional pool balances
- `coinject_measured_eta` - Measured η constant
- `coinject_measured_lambda` - Measured λ constant
- `coinject_unit_circle_constraint` - |μ|² validation
- `coinject_block_height` - Current block height

## Development Roadmap

- [ ] Transaction history view
- [ ] Address book
- [ ] QR code generation for addresses
- [ ] Hardware wallet integration
- [ ] Multi-language support
- [ ] Dark mode
- [ ] Mobile responsive improvements
- [ ] Real-time WebSocket updates

## License

Same as parent project

## Support

For issues and questions, see main repository.
