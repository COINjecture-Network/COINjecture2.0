#!/usr/bin/env pwsh
# COINjecture Multi-Node Network Launcher
# Starts 5 nodes with different types that connect to each other

param(
    [Parameter(Mandatory=$true)]
    [string]$HfToken
)

$HF_DATASET = "COINjecture/v5"
$BINARY = ".\target\release\coinject.exe"

# Check binary exists
if (-not (Test-Path $BINARY)) {
    Write-Host "ERROR: Binary not found at $BINARY" -ForegroundColor Red
    Write-Host "Run: cargo build --release --bin coinject" -ForegroundColor Yellow
    exit 1
}

Write-Host @"

    ╔═══════════════════════════════════════════════════════════════╗
    ║           COINjecture 5-Node Network Launcher                  ║
    ║                                                                ║
    ║   Node 1: 💻 Full (Bootstrap)   - Port 30333/9933              ║
    ║   Node 2: 💻 Full (Miner)       - Port 30334/9934              ║
    ║   Node 3: 💻 Full (Miner)       - Port 30335/9935              ║
    ║   Node 4: 🗄️ Archive            - Port 30336/9936              ║
    ║   Node 5: 🎯 Bounty Hunter      - Port 30337/9937              ║
    ╚═══════════════════════════════════════════════════════════════╝

"@ -ForegroundColor Cyan

# Clean up old data directories (optional - comment out to preserve chain state)
# Write-Host "Cleaning old data directories..." -ForegroundColor Yellow
# Remove-Item -Recurse -Force node1-data, node2-data, node3-data, node4-archive, node5-bounty -ErrorAction SilentlyContinue

# Start Node 1 first and get its PeerId
Write-Host "`n[1/5] Starting Node 1 (Bootstrap)..." -ForegroundColor Green

# Create a temp file to capture the PeerId
$node1Log = "$env:TEMP\node1_startup.log"

# Start Node 1 in background and wait for it to be ready
$node1Args = @(
    "--data-dir", ".\node1-data",
    "--p2p-addr", "/ip4/0.0.0.0/tcp/30333",
    "--rpc-addr", "0.0.0.0:9933",
    "--metrics-addr", "0.0.0.0:9090",
    "--mine",
    "--hf-token", $HfToken,
    "--hf-dataset-name", $HF_DATASET,
    "--verbose"
)

$node1 = Start-Process -FilePath $BINARY -ArgumentList $node1Args -PassThru -WindowStyle Normal
Start-Sleep -Seconds 5

# Wait for Node 1 to start listening
Write-Host "   Waiting for Node 1 to start..." -ForegroundColor Yellow
$maxWait = 30
$waited = 0
while ($waited -lt $maxWait) {
    $listening = netstat -ano | Select-String ":30333"
    if ($listening) {
        Write-Host "   Node 1 is listening on port 30333!" -ForegroundColor Green
        break
    }
    Start-Sleep -Seconds 1
    $waited++
}

if ($waited -ge $maxWait) {
    Write-Host "   WARNING: Node 1 may not have started properly" -ForegroundColor Yellow
}

# We need to manually specify the PeerId from Node 1
# This is the PeerId we captured earlier
$BOOTSTRAP_PEER_ID = "12D3KooWHyA8XJFrQTJpMGuxUw5JqkLk3bEKivnVJYx9ByDD37nF"
$BOOTNODE = "/ip4/127.0.0.1/tcp/30333/p2p/$BOOTSTRAP_PEER_ID"

Write-Host "`n   Bootstrap address: $BOOTNODE" -ForegroundColor Cyan

# Start Node 2
Write-Host "`n[2/5] Starting Node 2 (Full Miner)..." -ForegroundColor Green
$node2Args = @(
    "--data-dir", ".\node2-data",
    "--node-type", "full",
    "--p2p-addr", "/ip4/0.0.0.0/tcp/30334",
    "--rpc-addr", "0.0.0.0:9934",
    "--metrics-addr", "0.0.0.0:9091",
    "--mine",
    "--bootnodes", $BOOTNODE,
    "--hf-token", $HfToken,
    "--hf-dataset-name", $HF_DATASET,
    "--verbose"
)
$node2 = Start-Process -FilePath $BINARY -ArgumentList $node2Args -PassThru -WindowStyle Normal
Start-Sleep -Seconds 2

# Start Node 3
Write-Host "[3/5] Starting Node 3 (Full Miner)..." -ForegroundColor Green
$node3Args = @(
    "--data-dir", ".\node3-data",
    "--node-type", "full",
    "--p2p-addr", "/ip4/0.0.0.0/tcp/30335",
    "--rpc-addr", "0.0.0.0:9935",
    "--metrics-addr", "0.0.0.0:9092",
    "--mine",
    "--bootnodes", $BOOTNODE,
    "--hf-token", $HfToken,
    "--hf-dataset-name", $HF_DATASET,
    "--verbose"
)
$node3 = Start-Process -FilePath $BINARY -ArgumentList $node3Args -PassThru -WindowStyle Normal
Start-Sleep -Seconds 2

# Start Node 4 (Archive)
Write-Host "[4/5] Starting Node 4 (Archive)..." -ForegroundColor Green
$node4Args = @(
    "--data-dir", ".\node4-archive",
    "--node-type", "archive",
    "--p2p-addr", "/ip4/0.0.0.0/tcp/30336",
    "--rpc-addr", "0.0.0.0:9936",
    "--metrics-addr", "0.0.0.0:9093",
    "--mine",
    "--bootnodes", $BOOTNODE,
    "--hf-token", $HfToken,
    "--hf-dataset-name", $HF_DATASET,
    "--verbose"
)
$node4 = Start-Process -FilePath $BINARY -ArgumentList $node4Args -PassThru -WindowStyle Normal
Start-Sleep -Seconds 2

# Start Node 5 (Bounty Hunter)
Write-Host "[5/5] Starting Node 5 (Bounty Hunter)..." -ForegroundColor Green
$node5Args = @(
    "--data-dir", ".\node5-bounty",
    "--node-type", "bounty",
    "--bounty-hunter",
    "--p2p-addr", "/ip4/0.0.0.0/tcp/30337",
    "--rpc-addr", "0.0.0.0:9937",
    "--metrics-addr", "0.0.0.0:9094",
    "--mine",
    "--bootnodes", $BOOTNODE,
    "--hf-token", $HfToken,
    "--hf-dataset-name", $HF_DATASET,
    "--verbose"
)
$node5 = Start-Process -FilePath $BINARY -ArgumentList $node5Args -PassThru -WindowStyle Normal

Write-Host @"

╔═══════════════════════════════════════════════════════════════╗
║                    NETWORK STARTED!                            ║
╠═══════════════════════════════════════════════════════════════╣
║  5 nodes are now running in separate windows                   ║
║                                                                ║
║  RPC Endpoints:                                                ║
║    Node 1: http://localhost:9933                               ║
║    Node 2: http://localhost:9934                               ║
║    Node 3: http://localhost:9935                               ║
║    Node 4: http://localhost:9936                               ║
║    Node 5: http://localhost:9937                               ║
║                                                                ║
║  Dataset: https://huggingface.co/datasets/$HF_DATASET          ║
║                                                                ║
║  To stop: Close the node windows or press Ctrl+C in each       ║
╚═══════════════════════════════════════════════════════════════╝

"@ -ForegroundColor Green

# Show process IDs
Write-Host "Process IDs:" -ForegroundColor Cyan
Write-Host "  Node 1 (Bootstrap): $($node1.Id)" -ForegroundColor White
Write-Host "  Node 2 (Full):      $($node2.Id)" -ForegroundColor White
Write-Host "  Node 3 (Full):      $($node3.Id)" -ForegroundColor White
Write-Host "  Node 4 (Archive):   $($node4.Id)" -ForegroundColor White
Write-Host "  Node 5 (Bounty):    $($node5.Id)" -ForegroundColor White

Write-Host "`nWaiting for peer discovery..." -ForegroundColor Yellow
Start-Sleep -Seconds 10

# Check if ports are listening
Write-Host "`nChecking network status:" -ForegroundColor Cyan
$ports = @(30333, 30334, 30335, 30336, 30337)
foreach ($port in $ports) {
    $conn = netstat -ano | Select-String ":$port"
    if ($conn) {
        Write-Host "  Port $port : ✅ LISTENING" -ForegroundColor Green
    } else {
        Write-Host "  Port $port : ❌ NOT LISTENING" -ForegroundColor Red
    }
}

Write-Host "`nNetwork is running! Check the individual node windows for mining activity." -ForegroundColor Green

