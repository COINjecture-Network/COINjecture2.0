# COINjecture Network v5 Deployment Script
param(
    [Parameter(Mandatory=$true)]
    [string]$HfToken,
    [switch]$UseAdzdb
)

$HF_DATASET = "COINjecture/v5"
$IMAGE_NAME = "coinject-node:v5"
$NETWORK_NAME = "coinjecture-v5-network"

Write-Host ""
Write-Host "=======================================" -ForegroundColor Cyan
Write-Host "  COINjecture Network v5 Deployment" -ForegroundColor Cyan
Write-Host "=======================================" -ForegroundColor Cyan
Write-Host "  Dataset: $HF_DATASET" -ForegroundColor Yellow
Write-Host ""

# Build Docker image
Write-Host "Building Docker image..." -ForegroundColor Cyan
$DockerfilePath = if ($UseAdzdb) { "Dockerfile.adzdb" } else { "Dockerfile" }

# Use --network=host to bypass Docker networking issues
docker build --network=host -t $IMAGE_NAME -f $DockerfilePath .
if ($LASTEXITCODE -ne 0) {
    Write-Host "Docker build failed!" -ForegroundColor Red
    exit 1
}
Write-Host "Docker image built successfully" -ForegroundColor Green

# Create network
Write-Host ""
Write-Host "Setting up Docker network..." -ForegroundColor Cyan
docker network create $NETWORK_NAME 2>$null

# Stop existing containers
Write-Host ""
Write-Host "Stopping existing containers..." -ForegroundColor Cyan
docker stop coinjecture-v5-node1 coinjecture-v5-node2 coinjecture-v5-node3 2>$null
docker rm coinjecture-v5-node1 coinjecture-v5-node2 coinjecture-v5-node3 2>$null

# Start Node 1 (Bootstrap)
Write-Host ""
Write-Host "Starting Node 1 (Bootstrap)..." -ForegroundColor Yellow
docker run -d --name coinjecture-v5-node1 --network $NETWORK_NAME --restart unless-stopped -p 30333:30333 -p 9933:9933 -p 9090:9090 -v coinjecture-v5-data1:/data $IMAGE_NAME --data-dir /data --p2p-addr /ip4/0.0.0.0/tcp/30333 --rpc-addr 0.0.0.0:9933 --metrics-addr 0.0.0.0:9090 --mine --hf-token "$HfToken" --hf-dataset-name "$HF_DATASET" --verbose

Start-Sleep -Seconds 5

# Get bootstrap address
$ContainerIP = docker inspect -f "{{range.NetworkSettings.Networks}}{{.IPAddress}}{{end}}" coinjecture-v5-node1
$PeerIdLine = docker logs coinjecture-v5-node1 2>&1 | Select-String "Local peer id:" | Select-Object -First 1
if ($PeerIdLine -match "Local peer id: ([a-zA-Z0-9]+)") {
    $PeerId = $Matches[1]
    $BootstrapAddr = "/ip4/$ContainerIP/tcp/30333/p2p/$PeerId"
    Write-Host "Bootstrap address: $BootstrapAddr" -ForegroundColor Cyan
} else {
    $BootstrapAddr = "/dns4/coinjecture-v5-node1/tcp/30333"
    Write-Host "Using DNS bootstrap: $BootstrapAddr" -ForegroundColor Yellow
}

# Start Node 2
Write-Host ""
Write-Host "Starting Node 2 (Full)..." -ForegroundColor Yellow
docker run -d --name coinjecture-v5-node2 --network $NETWORK_NAME --restart unless-stopped -p 30334:30334 -p 9934:9933 -p 9091:9090 -v coinjecture-v5-data2:/data $IMAGE_NAME --data-dir /data --p2p-addr /ip4/0.0.0.0/tcp/30334 --rpc-addr 0.0.0.0:9933 --metrics-addr 0.0.0.0:9090 --mine --hf-token "$HfToken" --hf-dataset-name "$HF_DATASET" --bootnodes "$BootstrapAddr" --verbose

# Start Node 3
Write-Host ""
Write-Host "Starting Node 3 (Miner)..." -ForegroundColor Yellow
docker run -d --name coinjecture-v5-node3 --network $NETWORK_NAME --restart unless-stopped -p 30335:30335 -p 9935:9933 -p 9092:9090 -v coinjecture-v5-data3:/data $IMAGE_NAME --data-dir /data --p2p-addr /ip4/0.0.0.0/tcp/30335 --rpc-addr 0.0.0.0:9933 --metrics-addr 0.0.0.0:9090 --mine --hf-token "$HfToken" --hf-dataset-name "$HF_DATASET" --bootnodes "$BootstrapAddr" --verbose

# Verify
Write-Host ""
Write-Host "Verifying deployment..." -ForegroundColor Cyan
Start-Sleep -Seconds 3
docker ps --filter "name=coinjecture-v5"

Write-Host ""
Write-Host "=======================================" -ForegroundColor Green
Write-Host "  DEPLOYMENT COMPLETE!" -ForegroundColor Green
Write-Host "=======================================" -ForegroundColor Green
Write-Host ""
Write-Host "Dataset: https://huggingface.co/datasets/$HF_DATASET" -ForegroundColor Yellow
Write-Host ""
Write-Host "Node Endpoints:" -ForegroundColor Cyan
Write-Host "  Node 1: http://localhost:9933" -ForegroundColor White
Write-Host "  Node 2: http://localhost:9934" -ForegroundColor White
Write-Host "  Node 3: http://localhost:9935" -ForegroundColor White
Write-Host ""
Write-Host "View logs: docker logs -f coinjecture-v5-node1" -ForegroundColor DarkGray

