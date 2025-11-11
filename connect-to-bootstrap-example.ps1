# Example: Connect to Bootstrap Node
# Replace <PEER_ID> with the actual PeerId from the bootstrap node
Write-Host "Starting node that connects to bootstrap node..." -ForegroundColor Cyan
Write-Host ""
Write-Host "IMPORTANT: Replace <PEER_ID> in this script with the actual PeerId" -ForegroundColor Yellow
Write-Host "from your bootstrap node before running!" -ForegroundColor Yellow
Write-Host ""

# REPLACE THIS WITH YOUR BOOTSTRAP NODE'S PEER_ID
$BOOTSTRAP_PEER_ID = "<PEER_ID>"

if ($BOOTSTRAP_PEER_ID -eq "<PEER_ID>") {
    Write-Host "ERROR: You must set the BOOTSTRAP_PEER_ID variable first!" -ForegroundColor Red
    Write-Host ""
    Write-Host "Steps:" -ForegroundColor Yellow
    Write-Host "1. Start the bootstrap node using bootstrap-node.ps1" -ForegroundColor Yellow
    Write-Host "2. Look for the line: 'Network node PeerId: <PEER_ID>'" -ForegroundColor Yellow
    Write-Host "3. Copy that PeerId and replace <PEER_ID> in this script" -ForegroundColor Yellow
    Write-Host "4. Run this script again" -ForegroundColor Yellow
    Write-Host ""
    pause
    exit 1
}

# Create data directory
if (-not (Test-Path "testnet\node-with-bootstrap")) {
    New-Item -ItemType Directory -Path "testnet\node-with-bootstrap" | Out-Null
}

# Construct bootnode multiaddr
# For local testing: use 127.0.0.1
# For LAN/remote: use the actual IP address of the bootstrap node
$BOOTSTRAP_ADDR = "/ip4/127.0.0.1/tcp/30333/p2p/$BOOTSTRAP_PEER_ID"

Write-Host "Connecting to bootstrap node at: $BOOTSTRAP_ADDR" -ForegroundColor Green
Write-Host ""

# Run node with bootnode connection
& ".\target\release\coinject.exe" `
  --data-dir "testnet/node-with-bootstrap" `
  --p2p-addr "/ip4/0.0.0.0/tcp/30334" `
  --rpc-addr "127.0.0.1:9934" `
  --bootnodes "$BOOTSTRAP_ADDR" `
  --mine `
  --difficulty 3 `
  --block-time 30
