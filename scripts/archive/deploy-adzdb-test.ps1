# Deploy ADZDB test nodes to DigitalOcean droplets with HuggingFace v4 streaming
# Usage: .\deploy-adzdb-test.ps1

$ErrorActionPreference = "Stop"

# Configuration
$DROPLET1 = "143.110.139.166"
$DROPLET2 = "68.183.205.12"
$SSH_KEY = "$env:USERPROFILE\.ssh\COINjecture-Key"
$IMAGE_TAG = "v4.7.46-adzdb"
$IMAGE_NAME = "coinject-node:$IMAGE_TAG"
$CONTAINER_NAME = "coinject-adzdb"
$DATA_VOLUME = "coinject-adzdb-data"
$HF_TOKEN = "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"
$HF_DATASET = "COINjecture/NP_Solutions_v4"

Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  ADZDB Network Test - 2 Node Deployment" -ForegroundColor Cyan
Write-Host "  Streaming to: $HF_DATASET" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Build Docker image with ADZDB feature
Write-Host "[BUILD] Building Docker image with ADZDB feature..." -ForegroundColor Yellow
docker build -f Dockerfile.adzdb -t $IMAGE_NAME --platform linux/amd64 .

if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Docker build failed!" -ForegroundColor Red
    exit 1
}
Write-Host "[OK] Docker image built: $IMAGE_NAME" -ForegroundColor Green
Write-Host ""

# Save Docker image to tar file
Write-Host "[BUILD] Saving Docker image to tar..." -ForegroundColor Yellow
$TAR_PATH = "$env:TEMP\coinject-adzdb.tar"
docker save "$IMAGE_NAME" -o $TAR_PATH
$tarSize = [math]::Round((Get-Item $TAR_PATH).Length / 1MB, 2)
Write-Host "[OK] Image saved: $TAR_PATH - $tarSize MB" -ForegroundColor Green
Write-Host ""

# Create the remote deployment script
$REMOTE_SCRIPT_PATH = "$env:TEMP\deploy-remote.sh"

function Deploy-AdzdbNode {
    param(
        [string]$NodeIP,
        [string]$NodeName,
        [string]$NodePort,
        [string]$Bootnodes = ""
    )
    
    Write-Host "----------------------------------------------------------------" -ForegroundColor Cyan
    Write-Host "[DEPLOY] Deploying ADZDB node to $NodeName at $NodeIP" -ForegroundColor Yellow
    Write-Host "         P2P Port: $NodePort"
    if ($Bootnodes) {
        Write-Host "         Bootnodes: $Bootnodes"
    }
    Write-Host "----------------------------------------------------------------" -ForegroundColor Cyan
    
    # Transfer image to remote node
    Write-Host "[UPLOAD] Transferring image to $NodeIP..." -ForegroundColor Yellow
    scp -i $SSH_KEY -o StrictHostKeyChecking=no $TAR_PATH "root@${NodeIP}:/tmp/coinject-adzdb.tar"
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[ERROR] Failed to transfer image to $NodeIP" -ForegroundColor Red
        return $null
    }
    Write-Host "[OK] Image transferred" -ForegroundColor Green
    
    # Create remote script content
    $bootnodesFlag = ""
    if ($Bootnodes) {
        $bootnodesFlag = "--bootnodes `"$Bootnodes`""
    }
    
    $remoteCommands = @"
#!/bin/bash
set -e
echo '[LOAD] Loading Docker image...'
docker load -i /tmp/coinject-adzdb.tar
rm -f /tmp/coinject-adzdb.tar

echo '[STOP] Stopping existing container...'
docker stop $CONTAINER_NAME 2>/dev/null || true
docker rm $CONTAINER_NAME 2>/dev/null || true

for port in $NodePort 9934 9091; do
    fuser -k `$port/tcp 2>/dev/null || true
done
sleep 2

echo '[CLEAN] Cleaning data volume...'
docker volume rm $DATA_VOLUME 2>/dev/null || true
docker volume create $DATA_VOLUME

echo '[START] Starting ADZDB node...'
docker run -d --name $CONTAINER_NAME --restart unless-stopped -p ${NodePort}:${NodePort} -p 9934:9933 -p 9091:9090 -v ${DATA_VOLUME}:/data $IMAGE_NAME --data-dir /data --p2p-addr /ip4/0.0.0.0/tcp/$NodePort --rpc-addr 0.0.0.0:9933 --metrics-addr 0.0.0.0:9090 --mine --use-adzdb --hf-token "$HF_TOKEN" --hf-dataset-name "$HF_DATASET" $bootnodesFlag --verbose

sleep 5
if docker ps | grep -q $CONTAINER_NAME; then
    echo '[OK] Container started!'
    docker logs --tail 20 $CONTAINER_NAME
else
    echo '[ERROR] Container failed!'
    docker logs $CONTAINER_NAME
    exit 1
fi
"@

    # Write remote script to temp file
    $remoteCommands | Out-File -FilePath $REMOTE_SCRIPT_PATH -Encoding utf8 -Force
    
    # Transfer and execute script
    Write-Host "[EXEC] Executing deployment on $NodeIP..." -ForegroundColor Yellow
    scp -i $SSH_KEY -o StrictHostKeyChecking=no $REMOTE_SCRIPT_PATH "root@${NodeIP}:/tmp/deploy-remote.sh"
    ssh -i $SSH_KEY -o StrictHostKeyChecking=no "root@$NodeIP" "chmod +x /tmp/deploy-remote.sh && /tmp/deploy-remote.sh"
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[ERROR] Deployment to $NodeIP failed!" -ForegroundColor Red
        return $null
    }
    
    Write-Host "[OK] $NodeName deployment complete!" -ForegroundColor Green
    
    # Get the PeerId
    Write-Host "[INFO] Retrieving PeerId..." -ForegroundColor Yellow
    $peerId = ssh -i $SSH_KEY -o StrictHostKeyChecking=no "root@$NodeIP" "docker logs $CONTAINER_NAME 2>&1 | grep -oP 'PeerId: \K12D3KooW[A-Za-z0-9]+' | head -1"
    
    if ($peerId) {
        $peerId = $peerId.Trim()
        Write-Host "         PeerId: $peerId" -ForegroundColor Cyan
        Write-Host "         Bootnode: /ip4/$NodeIP/tcp/$NodePort/p2p/$peerId" -ForegroundColor Cyan
    }
    
    Write-Host ""
    return $peerId
}

# Deploy Node 1 first (bootstrap)
$node1PeerId = Deploy-AdzdbNode -NodeIP $DROPLET1 -NodeName "ADZDB Node 1 - Bootstrap" -NodePort "30334"

# Wait for Node 1 to be fully initialized
Write-Host "[WAIT] Waiting 10 seconds for Node 1 initialization..." -ForegroundColor Yellow
Start-Sleep -Seconds 10

# Get Node 1's PeerId if we didn't get it before
if (-not $node1PeerId) {
    Write-Host "[INFO] Getting Node 1 PeerId..." -ForegroundColor Yellow
    $node1PeerId = ssh -i $SSH_KEY -o StrictHostKeyChecking=no "root@$DROPLET1" "docker logs $CONTAINER_NAME 2>&1 | grep -oP 'PeerId: \K12D3KooW[A-Za-z0-9]+' | head -1"
    if ($node1PeerId) { $node1PeerId = $node1PeerId.Trim() }
}

$bootnode1 = ""
if ($node1PeerId) {
    Write-Host "[OK] Node 1 PeerId: $node1PeerId" -ForegroundColor Green
    $bootnode1 = "/ip4/$DROPLET1/tcp/30334/p2p/$node1PeerId"
} else {
    Write-Host "[WARN] Could not retrieve Node 1 PeerId" -ForegroundColor Yellow
}

# Deploy Node 2 with Node 1 as bootnode
$null = Deploy-AdzdbNode -NodeIP $DROPLET2 -NodeName "ADZDB Node 2" -NodePort "30334" -Bootnodes $bootnode1

# Clean up
Remove-Item -Path $TAR_PATH -Force -ErrorAction SilentlyContinue
Remove-Item -Path $REMOTE_SCRIPT_PATH -Force -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  ADZDB 2-Node Test Deployment Complete!" -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
Write-Host ""
Write-Host "HuggingFace Dataset: https://huggingface.co/datasets/$HF_DATASET" -ForegroundColor Cyan
Write-Host ""
Write-Host "Monitor commands:" -ForegroundColor Yellow
Write-Host "  ssh -i $SSH_KEY root@$DROPLET1 'docker logs -f $CONTAINER_NAME'"
Write-Host "  ssh -i $SSH_KEY root@$DROPLET2 'docker logs -f $CONTAINER_NAME'"
Write-Host ""
