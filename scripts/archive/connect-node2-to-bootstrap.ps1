# Connect Node 2 to Bootstrap Node
Write-Host "Starting Node 2 - Connected to Bootstrap..." -ForegroundColor Cyan
Write-Host ""

# IMPORTANT: This PeerId is from the current bootstrap node session
# If you restart the bootstrap node, you'll need to update this PeerId!
$BOOTSTRAP_PEER_ID = "12D3KooWMEc61BBu2UcLndvLBHQUMWCaKDTeuebar6FpLEfhLTrw"

# Create data directory
if (-not (Test-Path "testnet\node2")) {
    New-Item -ItemType Directory -Path "testnet\node2" | Out-Null
}

# Bootnode multiaddrs (for different network scenarios)
# For same machine:
$BOOTNODE_LOCAL = "/ip4/127.0.0.1/tcp/30333/p2p/$BOOTSTRAP_PEER_ID"
# For LAN (adjust IP to match your bootstrap node's LAN IP):
$BOOTNODE_LAN = "/ip4/192.168.1.160/tcp/30333/p2p/$BOOTSTRAP_PEER_ID"

Write-Host "Connecting to bootstrap node..." -ForegroundColor Green
Write-Host "Bootstrap PeerId: $BOOTSTRAP_PEER_ID" -ForegroundColor Yellow
Write-Host "Using local bootnode: $BOOTNODE_LOCAL" -ForegroundColor Yellow
Write-Host ""

# Run node with bootnode connection
& ".\target\release\coinject.exe" `
  --data-dir "testnet/node2" `
  --p2p-addr "/ip4/0.0.0.0/tcp/30334" `
  --rpc-addr "127.0.0.1:9934" `
  --bootnodes "$BOOTNODE_LOCAL" `
  --mine `
  --difficulty 3 `
  --block-time 30
