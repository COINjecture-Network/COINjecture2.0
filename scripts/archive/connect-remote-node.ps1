# Connect to Remote Bootstrap Node
# Use this script on a DIFFERENT machine to connect to the bootstrap node

Write-Host "═══════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  COINjecture Network B - Remote Node Connection" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""

# ============================================================================
# CONFIGURATION - UPDATE THESE VALUES!
# ============================================================================

# Replace with your bootstrap node's IP address
# Examples:
#   - Same machine: 127.0.0.1
#   - Same LAN: 192.168.1.100 (check with ipconfig on bootstrap machine)
#   - Internet/VPS: Your public IP address
$BOOTSTRAP_IP = "192.168.1.160"

# Replace with the PeerId from your bootstrap node
# Look for: "Network node PeerId: 12D3KooW..." in bootstrap node's output
$BOOTSTRAP_PEER_ID = "12D3KooWMEc61BBu2UcLndvLBHQUMWCaKDTeuebar6FpLEfhLTrw"

# Local node configuration
$DATA_DIR = "mynode"
$P2P_PORT = 30333
$RPC_PORT = 9933
$ENABLE_MINING = $true

# ============================================================================

# Validate configuration
if ($BOOTSTRAP_PEER_ID -eq "12D3KooW..." -or $BOOTSTRAP_PEER_ID.Length -lt 40) {
    Write-Host "❌ ERROR: You must update BOOTSTRAP_PEER_ID!" -ForegroundColor Red
    Write-Host ""
    Write-Host "Steps to fix:" -ForegroundColor Yellow
    Write-Host "1. On the bootstrap node machine, look for this line:" -ForegroundColor Yellow
    Write-Host "   'Network node PeerId: 12D3KooW...'" -ForegroundColor Yellow
    Write-Host "2. Copy that full PeerId" -ForegroundColor Yellow
    Write-Host "3. Replace the BOOTSTRAP_PEER_ID value in this script" -ForegroundColor Yellow
    Write-Host "4. Update BOOTSTRAP_IP if connecting from a different machine" -ForegroundColor Yellow
    Write-Host ""
    pause
    exit 1
}

# Construct bootnode multiaddr
$BOOTNODE_MULTIADDR = "/ip4/$BOOTSTRAP_IP/tcp/30333/p2p/$BOOTSTRAP_PEER_ID"

Write-Host "Configuration:" -ForegroundColor Green
Write-Host "  Bootstrap IP:      $BOOTSTRAP_IP" -ForegroundColor White
Write-Host "  Bootstrap PeerId:  $BOOTSTRAP_PEER_ID" -ForegroundColor White
Write-Host "  Bootnode Address:  $BOOTNODE_MULTIADDR" -ForegroundColor White
Write-Host "  Local Data Dir:    $DATA_DIR" -ForegroundColor White
Write-Host "  Local P2P Port:    $P2P_PORT" -ForegroundColor White
Write-Host "  Local RPC Port:    $RPC_PORT" -ForegroundColor White
Write-Host "  Mining Enabled:    $ENABLE_MINING" -ForegroundColor White
Write-Host ""

# Test connectivity to bootstrap node (optional but recommended)
Write-Host "Testing connection to bootstrap node..." -ForegroundColor Yellow
$testConnection = Test-NetConnection -ComputerName $BOOTSTRAP_IP -Port 30333 -InformationLevel Quiet -WarningAction SilentlyContinue

if ($testConnection) {
    Write-Host "✅ Bootstrap node is reachable on port 30333" -ForegroundColor Green
} else {
    Write-Host "⚠️  WARNING: Cannot reach bootstrap node on port 30333" -ForegroundColor Red
    Write-Host "   This might be a firewall issue or the node is not running." -ForegroundColor Red
    Write-Host ""
    Write-Host "Troubleshooting:" -ForegroundColor Yellow
    Write-Host "  1. Verify bootstrap node is running" -ForegroundColor Yellow
    Write-Host "  2. Check firewall allows TCP port 30333" -ForegroundColor Yellow
    Write-Host "  3. Verify BOOTSTRAP_IP is correct" -ForegroundColor Yellow
    Write-Host ""
    $continue = Read-Host "Continue anyway? (y/n)"
    if ($continue -ne "y" -and $continue -ne "Y") {
        exit 1
    }
}

Write-Host ""
Write-Host "Starting node..." -ForegroundColor Cyan
Write-Host ""

# Create data directory
if (-not (Test-Path $DATA_DIR)) {
    New-Item -ItemType Directory -Path $DATA_DIR | Out-Null
}

# Build command arguments
$args = @(
    "--data-dir", $DATA_DIR,
    "--p2p-addr", "/ip4/0.0.0.0/tcp/$P2P_PORT",
    "--rpc-addr", "127.0.0.1:$RPC_PORT",
    "--bootnodes", $BOOTNODE_MULTIADDR,
    "--difficulty", "3",
    "--block-time", "30"
)

if ($ENABLE_MINING) {
    $args += "--mine"
}

# Run node
& ".\target\release\coinject.exe" @args
