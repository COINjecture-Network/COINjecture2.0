# Deploy COINjecture v5 to 2 DigitalOcean Droplets
# Usage: .\deploy-v5-2nodes.ps1

$ErrorActionPreference = "Stop"

# ============================================
# CONFIGURE YOUR DROPLETS HERE
# ============================================
$DROPLET1_IP = "143.110.139.166"
$DROPLET2_IP = "68.183.205.12"
# ============================================

# IPs are pre-configured for COINjecture droplets

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  COINjecture v5 - 2 Node Deployment" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Node 1: $DROPLET1_IP (Bootstrap)" -ForegroundColor White
Write-Host "  Node 2: $DROPLET2_IP (Connects to Node 1)" -ForegroundColor White
Write-Host ""

# Deploy Node 1 (Bootstrap)
Write-Host "============================================" -ForegroundColor Yellow
Write-Host " DEPLOYING NODE 1 (Bootstrap)" -ForegroundColor Yellow
Write-Host "============================================" -ForegroundColor Yellow
& .\deploy-v5-droplet.ps1 -DropletIP $DROPLET1_IP
if ($LASTEXITCODE -ne 0) { exit 1 }

# Get Node 1 PeerId
Write-Host ""
Write-Host "[INFO] Getting Node 1 PeerId for bootnode config..." -ForegroundColor Yellow
Start-Sleep -Seconds 5
$node1PeerId = ssh -o StrictHostKeyChecking=no "root@$DROPLET1_IP" "docker logs coinject-v5 2>&1 | grep -oP 'PeerId: \K12D3KooW[A-Za-z0-9]+' | head -1"
if ($node1PeerId) {
    $node1PeerId = $node1PeerId.Trim()
    Write-Host "[OK] Node 1 PeerId: $node1PeerId" -ForegroundColor Green
} else {
    Write-Host "[WARN] Could not get Node 1 PeerId, deploying Node 2 without bootnode" -ForegroundColor Yellow
}

# Deploy Node 2 (connects to Node 1)
Write-Host ""
Write-Host "============================================" -ForegroundColor Yellow
Write-Host " DEPLOYING NODE 2 (Connects to Node 1)" -ForegroundColor Yellow
Write-Host "============================================" -ForegroundColor Yellow
if ($node1PeerId) {
    & .\deploy-v5-droplet.ps1 -DropletIP $DROPLET2_IP -BootnodeIP $DROPLET1_IP -BootnodePeerId $node1PeerId
} else {
    & .\deploy-v5-droplet.ps1 -DropletIP $DROPLET2_IP
}
if ($LASTEXITCODE -ne 0) { exit 1 }

# Get Node 2 PeerId
$node2PeerId = ssh -o StrictHostKeyChecking=no "root@$DROPLET2_IP" "docker logs coinject-v5 2>&1 | grep -oP 'PeerId: \K12D3KooW[A-Za-z0-9]+' | head -1"
if ($node2PeerId) { $node2PeerId = $node2PeerId.Trim() }

Write-Host ""
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  COINjecture v5 - 2 Node Network READY!" -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
Write-Host ""
Write-Host "  Node 1: $DROPLET1_IP" -ForegroundColor Cyan
if ($node1PeerId) { Write-Host "          PeerId: $node1PeerId" -ForegroundColor Gray }
Write-Host "          RPC: http://${DROPLET1_IP}:9933" -ForegroundColor Gray
Write-Host ""
Write-Host "  Node 2: $DROPLET2_IP" -ForegroundColor Cyan
if ($node2PeerId) { Write-Host "          PeerId: $node2PeerId" -ForegroundColor Gray }
Write-Host "          RPC: http://${DROPLET2_IP}:9933" -ForegroundColor Gray
Write-Host ""
Write-Host "  HuggingFace: https://huggingface.co/datasets/COINjecture/v5" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Monitor both nodes:" -ForegroundColor Yellow
Write-Host "    ssh root@$DROPLET1_IP 'docker logs -f coinject-v5'" -ForegroundColor White
Write-Host "    ssh root@$DROPLET2_IP 'docker logs -f coinject-v5'" -ForegroundColor White
Write-Host ""

