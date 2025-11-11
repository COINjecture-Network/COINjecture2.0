# Faucet Node - Mines with faucet address for transaction testing
Write-Host "Starting Network B Faucet Node (Miner)..." -ForegroundColor Cyan
Write-Host ""

# Create data directory
if (-not (Test-Path "testnet\faucet")) {
    New-Item -ItemType Directory -Path "testnet\faucet" | Out-Null
}

# Faucet address
$faucetAddress = "e0e2547212b9273e006f7731340c38c0cb6c81fa5b5bd7c385212d76adf0afcd"

# Run node with faucet as miner
& ".\target\release\coinject.exe" `
  --data-dir "testnet/faucet" `
  --p2p-addr "/ip4/0.0.0.0/tcp/30333" `
  --rpc-addr "127.0.0.1:9933" `
  --mine `
  --miner-address $faucetAddress `
  --difficulty 3 `
  --block-time 30
