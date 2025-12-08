# Deploy COINjecture v5 to DigitalOcean Droplet
# Builds on remote Linux server (no cross-compilation needed)
# Usage: .\deploy-v5-droplet.ps1 -DropletIP "your.droplet.ip"

param(
    [Parameter(Mandatory=$true)]
    [string]$DropletIP,
    
    [Parameter(Mandatory=$false)]
    [string]$BootnodeIP = "",
    
    [Parameter(Mandatory=$false)]
    [string]$BootnodePeerId = "",
    
    [Parameter(Mandatory=$false)]
    [int]$P2PPort = 30333,
    
    [Parameter(Mandatory=$false)]
    [int]$RPCPort = 9933,
    
    [Parameter(Mandatory=$false)]
    [int]$MetricsPort = 9090
)

$ErrorActionPreference = "Stop"

# Configuration
$IMAGE_TAG = "v5.0.0"
$IMAGE_NAME = "coinject-node:$IMAGE_TAG"
$CONTAINER_NAME = "coinject-v5"
$DATA_VOLUME = "coinject-v5-data"
$HF_TOKEN = "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"
$HF_DATASET = "COINjecture/v5"

# Helper function to write Unix-style shell scripts (no BOM, LF line endings)
function Write-UnixScript {
    param([string]$Path, [string]$Content)
    $unixContent = $Content -replace "`r`n", "`n"
    [System.IO.File]::WriteAllText($Path, $unixContent, [System.Text.UTF8Encoding]::new($false))
}

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  COINjecture v5 Droplet Deployment" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Target:      $DropletIP" -ForegroundColor White
Write-Host "  P2P Port:    $P2PPort" -ForegroundColor White
Write-Host "  RPC Port:    $RPCPort" -ForegroundColor White
Write-Host "  Dataset:     $HF_DATASET" -ForegroundColor White
if ($BootnodeIP) {
    Write-Host "  Bootnode:    $BootnodeIP" -ForegroundColor White
}
Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Test SSH connection
Write-Host "[TEST] Testing SSH connection to $DropletIP..." -ForegroundColor Yellow
try {
    ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 "root@$DropletIP" "echo 'SSH OK'"
    Write-Host "[OK] SSH connection successful" -ForegroundColor Green
} catch {
    Write-Host "[ERROR] Cannot connect to $DropletIP via SSH" -ForegroundColor Red
    Write-Host "  Make sure:" -ForegroundColor Yellow
    Write-Host "    1. The droplet is running" -ForegroundColor Yellow
    Write-Host "    2. Your SSH key is added to the droplet" -ForegroundColor Yellow
    Write-Host "    3. The IP address is correct" -ForegroundColor Yellow
    exit 1
}
Write-Host ""

# Step 2: Create tar archive of source code
Write-Host "[PACK] Creating source code archive..." -ForegroundColor Yellow
$TAR_PATH = "v5-deploy.tar"

# Remove old tar if exists
if (Test-Path $TAR_PATH) { Remove-Item $TAR_PATH }

# Create archive excluding unnecessary files
tar -cf $TAR_PATH `
    --exclude="target" `
    --exclude="*.log" `
    --exclude="node*-data" `
    --exclude="*.tar" `
    --exclude=".git" `
    --exclude="web" `
    --exclude="web-wallet" `
    Cargo.toml Cargo.lock Dockerfile.adzdb `
    core consensus network state mempool rpc tokenomics node wallet `
    marketplace-export huggingface adzdb mobile-sdk

if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Failed to create archive!" -ForegroundColor Red
    exit 1
}
$tarSize = [math]::Round((Get-Item $TAR_PATH).Length / 1MB, 2)
Write-Host "[OK] Archive created: $TAR_PATH ($tarSize MB)" -ForegroundColor Green
Write-Host ""

# Step 3: Transfer to droplet
Write-Host "[UPLOAD] Transferring source to $DropletIP..." -ForegroundColor Yellow
scp -o StrictHostKeyChecking=no $TAR_PATH "root@${DropletIP}:/tmp/v5-deploy.tar"
if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Failed to transfer archive!" -ForegroundColor Red
    exit 1
}
Write-Host "[OK] Source transferred" -ForegroundColor Green
Write-Host ""

# Step 4: Build on remote
Write-Host "[BUILD] Building Docker image on $DropletIP..." -ForegroundColor Yellow
Write-Host "        (This takes 5-10 minutes on first build)" -ForegroundColor Gray

$buildScript = @"
#!/bin/bash
set -e

echo '[PREP] Preparing build environment...'
cd /tmp
rm -rf /tmp/coinject-v5-build
mkdir -p /tmp/coinject-v5-build
cd /tmp/coinject-v5-build
tar -xf /tmp/v5-deploy.tar

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo '[DOCKER] Installing Docker...'
    apt-get update
    apt-get install -y ca-certificates curl gnupg lsb-release
    install -m 0755 -d /etc/apt/keyrings
    curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg
    chmod a+r /etc/apt/keyrings/docker.gpg
    echo "deb [arch=`$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu `$(. /etc/os-release && echo `$VERSION_CODENAME) stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null
    apt-get update
    apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin
    echo '[OK] Docker installed!'
fi

echo '[BUILD] Starting Docker build...'
docker build -f Dockerfile.adzdb -t $IMAGE_NAME .

echo '[OK] Build complete!'
docker images | grep coinject
"@

$buildScriptPath = "$env:TEMP\build-v5.sh"
Write-UnixScript -Path $buildScriptPath -Content $buildScript
scp -o StrictHostKeyChecking=no $buildScriptPath "root@${DropletIP}:/tmp/build-v5.sh"
ssh -o StrictHostKeyChecking=no "root@$DropletIP" "chmod +x /tmp/build-v5.sh && /tmp/build-v5.sh"

if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Remote build failed!" -ForegroundColor Red
    exit 1
}
Write-Host "[OK] Docker image built successfully" -ForegroundColor Green
Write-Host ""

# Step 5: Deploy container
Write-Host "[DEPLOY] Deploying COINjecture v5 node..." -ForegroundColor Yellow

$bootnodesArg = ""
if ($BootnodeIP -and $BootnodePeerId) {
    $bootnodesArg = "--bootnodes /ip4/$BootnodeIP/tcp/$P2PPort/p2p/$BootnodePeerId"
    Write-Host "         With bootnode: $BootnodeIP" -ForegroundColor Gray
}

$deployScript = @"
#!/bin/bash
set -e

echo '[STOP] Stopping existing container...'
docker stop $CONTAINER_NAME 2>/dev/null || true
docker rm $CONTAINER_NAME 2>/dev/null || true

echo '[CLEANUP] Freeing ports...'
for port in $P2PPort $RPCPort $MetricsPort; do
    fuser -k `$port/tcp 2>/dev/null || true
done
sleep 2

echo '[VOLUME] Preparing data volume...'
docker volume rm $DATA_VOLUME 2>/dev/null || true
docker volume create $DATA_VOLUME

echo '[START] Starting COINjecture v5 node...'
docker run -d \
    --name $CONTAINER_NAME \
    --restart unless-stopped \
    -p ${P2PPort}:$P2PPort \
    -p ${RPCPort}:9933 \
    -p ${MetricsPort}:9090 \
    -v ${DATA_VOLUME}:/data \
    $IMAGE_NAME \
    --data-dir /data \
    --p2p-addr /ip4/0.0.0.0/tcp/$P2PPort \
    --rpc-addr 0.0.0.0:9933 \
    --metrics-addr 0.0.0.0:9090 \
    --mine \
    --use-adzdb \
    --hf-token "$HF_TOKEN" \
    --hf-dataset-name "$HF_DATASET" \
    --verbose \
    $bootnodesArg

echo '[WAIT] Waiting for node to start...'
sleep 5

if docker ps | grep -q $CONTAINER_NAME; then
    echo '[OK] Node started successfully!'
    echo ''
    echo '=== Recent Logs ==='
    docker logs --tail 50 $CONTAINER_NAME
else
    echo '[ERROR] Node failed to start!'
    docker logs $CONTAINER_NAME
    exit 1
fi
"@

$deployScriptPath = "$env:TEMP\deploy-v5.sh"
Write-UnixScript -Path $deployScriptPath -Content $deployScript
scp -o StrictHostKeyChecking=no $deployScriptPath "root@${DropletIP}:/tmp/deploy-v5.sh"
ssh -o StrictHostKeyChecking=no "root@$DropletIP" "chmod +x /tmp/deploy-v5.sh && /tmp/deploy-v5.sh"

if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Deployment failed!" -ForegroundColor Red
    exit 1
}

# Get PeerId
Write-Host ""
Write-Host "[INFO] Getting node PeerId..." -ForegroundColor Yellow
Start-Sleep -Seconds 3
$peerId = ssh -o StrictHostKeyChecking=no "root@$DropletIP" "docker logs $CONTAINER_NAME 2>&1 | grep -oP 'PeerId: \K12D3KooW[A-Za-z0-9]+' | head -1"
if ($peerId) {
    $peerId = $peerId.Trim()
}

# Cleanup local tar
Remove-Item $TAR_PATH -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  COINjecture v5 Deployment Complete!" -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
Write-Host ""
Write-Host "  Node IP:       $DropletIP" -ForegroundColor Cyan
if ($peerId) {
    Write-Host "  Peer ID:       $peerId" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "  Full Multiaddr:" -ForegroundColor Yellow
    Write-Host "  /ip4/$DropletIP/tcp/$P2PPort/p2p/$peerId" -ForegroundColor White
}
Write-Host ""
Write-Host "  RPC Endpoint:  http://${DropletIP}:$RPCPort" -ForegroundColor Cyan
Write-Host "  HuggingFace:   https://huggingface.co/datasets/$HF_DATASET" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Monitor logs:" -ForegroundColor Yellow
Write-Host "    ssh root@$DropletIP 'docker logs -f $CONTAINER_NAME'" -ForegroundColor White
Write-Host ""
Write-Host "  Stop node:" -ForegroundColor Yellow
Write-Host "    ssh root@$DropletIP 'docker stop $CONTAINER_NAME'" -ForegroundColor White
Write-Host ""

