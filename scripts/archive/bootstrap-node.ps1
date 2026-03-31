# Bootstrap Node for COINjecture Network B
# This node serves as a discovery point for other nodes in the network
Write-Host "Starting COINjecture Network B Bootstrap Node..." -ForegroundColor Green
Write-Host ""
Write-Host "This node will act as a P2P bootstrap/discovery node for the network." -ForegroundColor Yellow
Write-Host "Other nodes can connect to this node using the --bootnodes parameter." -ForegroundColor Yellow
Write-Host ""

# Create data directory
if (-not (Test-Path "bootstrap")) {
    New-Item -ItemType Directory -Path "bootstrap" | Out-Null
}

# Run bootstrap node
# - No mining by default (can be enabled with --mine flag)
# - Fixed P2P port for predictable connection
# - RPC enabled for monitoring
Write-Host "Starting node..." -ForegroundColor Cyan
Write-Host "P2P Port: 30333" -ForegroundColor Cyan
Write-Host "RPC Port: 9933" -ForegroundColor Cyan
Write-Host ""
Write-Host "IMPORTANT: After the node starts, look for the 'Network node PeerId' line." -ForegroundColor Magenta
Write-Host "You'll need this PeerId to construct the bootnode multiaddr for other nodes." -ForegroundColor Magenta
Write-Host ""
Write-Host "Example bootnode format:" -ForegroundColor Yellow
Write-Host "  /ip4/<IP_ADDRESS>/tcp/30333/p2p/<PEER_ID>" -ForegroundColor Yellow
Write-Host ""
Write-Host "For local network (same machine):" -ForegroundColor Yellow
Write-Host "  /ip4/127.0.0.1/tcp/30333/p2p/<PEER_ID>" -ForegroundColor Yellow
Write-Host ""
Write-Host "For LAN/remote:" -ForegroundColor Yellow
Write-Host "  /ip4/<YOUR_PUBLIC_IP>/tcp/30333/p2p/<PEER_ID>" -ForegroundColor Yellow
Write-Host ""
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Green
Write-Host ""

& ".\target\release\coinject.exe" `
  --data-dir "bootstrap" `
  --p2p-addr "/ip4/0.0.0.0/tcp/30333" `
  --rpc-addr "127.0.0.1:9933" `
  --difficulty 3 `
  --block-time 30
