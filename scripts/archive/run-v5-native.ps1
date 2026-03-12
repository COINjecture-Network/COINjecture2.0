# COINjecture v5 Native Deployment (No Docker!)
param(
    [Parameter(Mandatory=$true)]
    [string]$HfToken,
    [int]$NodeCount = 1
)

$HF_DATASET = "COINjecture/v5"

Write-Host ""
Write-Host "=======================================" -ForegroundColor Cyan
Write-Host "  COINjecture v5 - Native Build" -ForegroundColor Cyan
Write-Host "=======================================" -ForegroundColor Cyan
Write-Host "  Dataset: $HF_DATASET" -ForegroundColor Yellow
Write-Host ""

# Build the node
Write-Host "Building coinject node (this may take a few minutes)..." -ForegroundColor Cyan
cargo build --release --bin coinject

if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}

Write-Host "Build successful!" -ForegroundColor Green
Write-Host ""

# Create data directories
$DataDir1 = ".\node1-data"
$DataDir2 = ".\node2-data" 
$DataDir3 = ".\node3-data"

if (!(Test-Path $DataDir1)) { New-Item -ItemType Directory -Path $DataDir1 | Out-Null }

# Start Node 1
Write-Host "Starting Node 1 (Bootstrap + Miner)..." -ForegroundColor Yellow
$Binary = ".\target\release\coinject.exe"

$Node1Args = @(
    "--data-dir", $DataDir1,
    "--p2p-addr", "/ip4/0.0.0.0/tcp/30333",
    "--rpc-addr", "0.0.0.0:9933",
    "--metrics-addr", "0.0.0.0:9090",
    "--mine",
    "--hf-token", $HfToken,
    "--hf-dataset-name", $HF_DATASET,
    "--verbose"
)

Write-Host ""
Write-Host "=======================================" -ForegroundColor Green
Write-Host "  STARTING NODE" -ForegroundColor Green  
Write-Host "=======================================" -ForegroundColor Green
Write-Host ""
Write-Host "Dataset: https://huggingface.co/datasets/$HF_DATASET" -ForegroundColor Yellow
Write-Host "RPC: http://localhost:9933" -ForegroundColor Cyan
Write-Host ""
Write-Host "Press Ctrl+C to stop" -ForegroundColor DarkGray
Write-Host ""

# Run the node (foreground)
& $Binary @Node1Args

