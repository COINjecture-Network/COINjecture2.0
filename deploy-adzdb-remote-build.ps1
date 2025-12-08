# Deploy ADZDB test nodes - Build on remote droplet
# Avoids Docker Desktop networking issues
$ErrorActionPreference = "Stop"

# Configuration
$DROPLET1 = "143.110.139.166"
$DROPLET2 = "68.183.205.12"
# SSH will use default key from ssh-agent
$IMAGE_TAG = "v4.7.46-adzdb"
$IMAGE_NAME = "coinject-node:$IMAGE_TAG"
$CONTAINER_NAME = "coinject-adzdb"
$DATA_VOLUME = "coinject-adzdb-data"
$HF_TOKEN = "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"
$HF_DATASET = "COINjecture/NP_Solutions_v4"

# Helper function to write Unix-style shell scripts (no BOM, LF line endings)
function Write-UnixScript {
    param([string]$Path, [string]$Content)
    $unixContent = $Content -replace "`r`n", "`n"
    [System.IO.File]::WriteAllText($Path, $unixContent, [System.Text.UTF8Encoding]::new($false))
}

Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  ADZDB Network Test - Remote Build Deployment" -ForegroundColor Cyan
Write-Host "  Streaming to: $HF_DATASET" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Create tar archive of source code
Write-Host "[PACK] Creating source code archive..." -ForegroundColor Yellow
$TAR_PATH = "code-sync.tar"
tar -cf $TAR_PATH --exclude="target" --exclude="*.log" --exclude="data" --exclude="node2-data" Cargo.toml Cargo.lock Dockerfile.adzdb core consensus network state mempool rpc tokenomics node wallet marketplace-export huggingface adzdb

if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Failed to create archive!" -ForegroundColor Red
    exit 1
}
$tarSize = [math]::Round((Get-Item $TAR_PATH).Length / 1MB, 2)
Write-Host "[OK] Archive created: $TAR_PATH - $tarSize MB" -ForegroundColor Green
Write-Host ""

# Step 2: Transfer to Droplet 1 and build there
Write-Host "[UPLOAD] Transferring source to $DROPLET1..." -ForegroundColor Yellow
scp -o StrictHostKeyChecking=no $TAR_PATH "root@${DROPLET1}:/tmp/code-sync.tar"
if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Failed to transfer archive!" -ForegroundColor Red
    exit 1
}
Write-Host "[OK] Source transferred" -ForegroundColor Green
Write-Host ""

# Step 3: Build on remote
Write-Host "[BUILD] Building Docker image on $DROPLET1 (this takes 5-10 minutes)..." -ForegroundColor Yellow

$buildScript = @"
#!/bin/bash
set -e
cd /tmp
rm -rf /tmp/coinject-build
mkdir -p /tmp/coinject-build
cd /tmp/coinject-build
tar -xf /tmp/code-sync.tar

echo '[BUILD] Starting Docker build...'
docker build -f Dockerfile.adzdb -t $IMAGE_NAME .

echo '[SAVE] Saving image for transfer...'
docker save $IMAGE_NAME -o /tmp/coinject-adzdb-image.tar
ls -lh /tmp/coinject-adzdb-image.tar
echo '[OK] Build complete!'
"@

$buildScriptPath = "$env:TEMP\build-remote.sh"
Write-UnixScript -Path $buildScriptPath -Content $buildScript
scp -o StrictHostKeyChecking=no $buildScriptPath "root@${DROPLET1}:/tmp/build-remote.sh"
ssh -o StrictHostKeyChecking=no "root@$DROPLET1" "chmod +x /tmp/build-remote.sh && /tmp/build-remote.sh"

if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Remote build failed!" -ForegroundColor Red
    exit 1
}
Write-Host "[OK] Docker image built on $DROPLET1" -ForegroundColor Green
Write-Host ""

# Step 4: Deploy Node 1
Write-Host "[DEPLOY] Deploying Node 1 on $DROPLET1..." -ForegroundColor Yellow

$deployNode1 = @"
#!/bin/bash
set -e
echo '[STOP] Stopping existing container...'
docker stop $CONTAINER_NAME 2>/dev/null || true
docker rm $CONTAINER_NAME 2>/dev/null || true

for port in 30334 9934 9091; do
    fuser -k `$port/tcp 2>/dev/null || true
done
sleep 2

echo '[CLEAN] Cleaning data volume...'
docker volume rm $DATA_VOLUME 2>/dev/null || true
docker volume create $DATA_VOLUME

echo '[START] Starting ADZDB Node 1...'
docker run -d --name $CONTAINER_NAME --restart unless-stopped \
    -p 30334:30334 -p 9934:9933 -p 9091:9090 \
    -v ${DATA_VOLUME}:/data \
    $IMAGE_NAME \
    --data-dir /data \
    --p2p-addr /ip4/0.0.0.0/tcp/30334 \
    --rpc-addr 0.0.0.0:9933 \
    --metrics-addr 0.0.0.0:9090 \
    --mine \
    --use-adzdb \
    --hf-token "$HF_TOKEN" \
    --hf-dataset-name "$HF_DATASET" \
    --verbose

sleep 5
if docker ps | grep -q $CONTAINER_NAME; then
    echo '[OK] Node 1 started!'
    docker logs --tail 30 $CONTAINER_NAME
else
    echo '[ERROR] Node 1 failed!'
    docker logs $CONTAINER_NAME
    exit 1
fi
"@

$deployNode1Path = "$env:TEMP\deploy-node1.sh"
Write-UnixScript -Path $deployNode1Path -Content $deployNode1
scp -o StrictHostKeyChecking=no $deployNode1Path "root@${DROPLET1}:/tmp/deploy-node1.sh"
ssh -o StrictHostKeyChecking=no "root@$DROPLET1" "chmod +x /tmp/deploy-node1.sh && /tmp/deploy-node1.sh"

if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Node 1 deployment failed!" -ForegroundColor Red
    exit 1
}

# Get Node 1 PeerId
Write-Host "[INFO] Getting Node 1 PeerId..." -ForegroundColor Yellow
Start-Sleep -Seconds 5
$node1PeerId = ssh -o StrictHostKeyChecking=no "root@$DROPLET1" "docker logs $CONTAINER_NAME 2>&1 | grep -oP 'PeerId: \K12D3KooW[A-Za-z0-9]+' | head -1"
if ($node1PeerId) {
    $node1PeerId = $node1PeerId.Trim()
    Write-Host "[OK] Node 1 PeerId: $node1PeerId" -ForegroundColor Green
    $bootnode1 = "/ip4/$DROPLET1/tcp/30334/p2p/$node1PeerId"
    Write-Host "     Bootnode: $bootnode1" -ForegroundColor Cyan
} else {
    Write-Host "[WARN] Could not get PeerId yet, continuing..." -ForegroundColor Yellow
    $bootnode1 = ""
}
Write-Host ""

# Step 5: Transfer image to Droplet 2
Write-Host "[TRANSFER] Transferring image from $DROPLET1 to $DROPLET2..." -ForegroundColor Yellow
ssh -o StrictHostKeyChecking=no "root@$DROPLET1" "scp -o StrictHostKeyChecking=no /tmp/coinject-adzdb-image.tar root@${DROPLET2}:/tmp/coinject-adzdb-image.tar"

if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Image transfer failed!" -ForegroundColor Red
    exit 1
}
Write-Host "[OK] Image transferred to $DROPLET2" -ForegroundColor Green
Write-Host ""

# Step 6: Deploy Node 2
Write-Host "[DEPLOY] Deploying Node 2 on $DROPLET2..." -ForegroundColor Yellow

$bootnodesArg = ""
if ($bootnode1) {
    $bootnodesArg = "--bootnodes `"$bootnode1`""
}

$deployNode2 = @"
#!/bin/bash
set -e
echo '[LOAD] Loading Docker image...'
docker load -i /tmp/coinject-adzdb-image.tar
rm -f /tmp/coinject-adzdb-image.tar

echo '[STOP] Stopping existing container...'
docker stop $CONTAINER_NAME 2>/dev/null || true
docker rm $CONTAINER_NAME 2>/dev/null || true

for port in 30334 9934 9091; do
    fuser -k `$port/tcp 2>/dev/null || true
done
sleep 2

echo '[CLEAN] Cleaning data volume...'
docker volume rm $DATA_VOLUME 2>/dev/null || true
docker volume create $DATA_VOLUME

echo '[START] Starting ADZDB Node 2...'
docker run -d --name $CONTAINER_NAME --restart unless-stopped \
    -p 30334:30334 -p 9934:9933 -p 9091:9090 \
    -v ${DATA_VOLUME}:/data \
    $IMAGE_NAME \
    --data-dir /data \
    --p2p-addr /ip4/0.0.0.0/tcp/30334 \
    --rpc-addr 0.0.0.0:9933 \
    --metrics-addr 0.0.0.0:9090 \
    --mine \
    --use-adzdb \
    --hf-token "$HF_TOKEN" \
    --hf-dataset-name "$HF_DATASET" \
    $bootnodesArg \
    --verbose

sleep 5
if docker ps | grep -q $CONTAINER_NAME; then
    echo '[OK] Node 2 started!'
    docker logs --tail 30 $CONTAINER_NAME
else
    echo '[ERROR] Node 2 failed!'
    docker logs $CONTAINER_NAME
    exit 1
fi
"@

$deployNode2Path = "$env:TEMP\deploy-node2.sh"
Write-UnixScript -Path $deployNode2Path -Content $deployNode2
scp -o StrictHostKeyChecking=no $deployNode2Path "root@${DROPLET2}:/tmp/deploy-node2.sh"
ssh -o StrictHostKeyChecking=no "root@$DROPLET2" "chmod +x /tmp/deploy-node2.sh && /tmp/deploy-node2.sh"

if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Node 2 deployment failed!" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  ADZDB 2-Node Test Deployment Complete!" -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
Write-Host ""
Write-Host "HuggingFace Dataset: https://huggingface.co/datasets/$HF_DATASET" -ForegroundColor Cyan
Write-Host ""
Write-Host "Monitor commands:" -ForegroundColor Yellow
Write-Host "  ssh root@$DROPLET1 'docker logs -f $CONTAINER_NAME'"
Write-Host "  ssh root@$DROPLET2 'docker logs -f $CONTAINER_NAME'"
Write-Host ""
