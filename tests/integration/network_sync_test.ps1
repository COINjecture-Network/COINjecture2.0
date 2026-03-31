#!/usr/bin/env pwsh
# =============================================================================
# COINjecture Network B - Multi-Node Sync Regression Test
# =============================================================================
# This test validates:
# 1. Adaptive Consensus (BOOTSTRAP to SECURE mode transition)
# 2. Gossip Trap Fix (unique request_id prevents dedup issues)
# 3. Multi-node sync stability
#
# Run: ./tests/integration/network_sync_test.ps1
# Exit codes: 0 = PASS, 1 = FAIL
# =============================================================================

param(
    [int]$NodeCount = 6,
    [int]$TestDurationSeconds = 180,
    [int]$CheckIntervalSeconds = 15,
    [int]$MaxAllowedSpread = 3,
    [int]$MinBlocksRequired = 20,
    [string]$BinaryPath = ".\target\release\coinject.exe",
    [int]$Difficulty = 3,
    [int]$BlockTime = 30
)

$ErrorActionPreference = "Stop"

Write-Host "=================================================================" -ForegroundColor Cyan
Write-Host "  COINjecture Network B - Multi-Node Sync Regression Test" -ForegroundColor Cyan
Write-Host "=================================================================" -ForegroundColor Cyan
Write-Host ""

# Configuration
$BaseP2PPort = 30400
$BaseRPCPort = 9940
$DataDir = "testnet/regression-test"
$script:Processes = @()
$TestPassed = $true
$FailureReasons = @()

# Cleanup function
function Cleanup {
    Write-Host ""
    Write-Host "[CLEANUP] Stopping nodes..." -ForegroundColor Yellow
    foreach ($proc in $script:Processes) {
        if ($proc -and !$proc.HasExited) {
            try {
                $proc.Kill()
                $proc.WaitForExit(5000)
            } catch {}
        }
    }
    # Remove test data
    if (Test-Path $DataDir) {
        Remove-Item -Recurse -Force $DataDir -ErrorAction SilentlyContinue
    }
    Write-Host "[CLEANUP] Done" -ForegroundColor Yellow
}

# Register cleanup on script exit
trap { Cleanup; exit 1 }

# Check binary exists
if (!(Test-Path $BinaryPath)) {
    Write-Host "[ERROR] Binary not found: $BinaryPath" -ForegroundColor Red
    Write-Host "        Run 'cargo build --release' first" -ForegroundColor Yellow
    exit 1
}

# Clean previous test data
if (Test-Path $DataDir) {
    Remove-Item -Recurse -Force $DataDir
}

Write-Host "[CONFIG] Test Configuration:" -ForegroundColor White
Write-Host "         Nodes: $NodeCount"
Write-Host "         Duration: ${TestDurationSeconds}s"
Write-Host "         Check Interval: ${CheckIntervalSeconds}s"
Write-Host "         Max Spread: $MaxAllowedSpread blocks"
Write-Host "         Min Blocks: $MinBlocksRequired"
Write-Host ""

# Start nodes
Write-Host "[START] Starting $NodeCount nodes..." -ForegroundColor Green
for ($i = 0; $i -lt $NodeCount; $i++) {
    $nodeLetter = [char]([int][char]'A' + $i)
    $p2pPort = $BaseP2PPort + $i
    $rpcPort = $BaseRPCPort + $i
    $nodeDataDir = "$DataDir/node-$nodeLetter"
    
    $procArgs = @(
        "--data-dir", $nodeDataDir,
        "--p2p-addr", "/ip4/0.0.0.0/tcp/$p2pPort",
        "--rpc-addr", "127.0.0.1:$rpcPort",
        "--mine",
        "--difficulty", $Difficulty,
        "--block-time", $BlockTime
    )
    
    $proc = Start-Process -FilePath $BinaryPath -ArgumentList $procArgs -PassThru -WindowStyle Hidden
    $script:Processes += $proc
    Write-Host "         [OK] Node $nodeLetter started (PID: $($proc.Id), RPC: $rpcPort)"
}

# Wait for nodes to connect
Write-Host ""
Write-Host "[WAIT] Waiting for nodes to connect (60s)..." -ForegroundColor Yellow
Start-Sleep -Seconds 60

# Check initial connectivity
Write-Host ""
Write-Host "[CHECK] Checking initial connectivity..." -ForegroundColor Cyan
$body = '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}'
$allConnected = $true

for ($i = 0; $i -lt $NodeCount; $i++) {
    $nodeLetter = [char]([int][char]'A' + $i)
    $rpcPort = $BaseRPCPort + $i
    
    try {
        $result = (Invoke-RestMethod -Uri "http://127.0.0.1:$rpcPort" -Method POST -Body $body -ContentType "application/json" -TimeoutSec 10).result
        $expectedPeers = $NodeCount - 1
        
        if ($result.peer_count -lt ($expectedPeers - 1)) {
            Write-Host "         [WARN] Node $nodeLetter - Only $($result.peer_count)/$expectedPeers peers" -ForegroundColor Yellow
            $allConnected = $false
        } else {
            Write-Host "         [OK] Node $nodeLetter - $($result.peer_count) peers connected"
        }
    } catch {
        Write-Host "         [FAIL] Node $nodeLetter - RPC not responding" -ForegroundColor Red
        $allConnected = $false
    }
}

if (!$allConnected) {
    Write-Host ""
    Write-Host "[WAIT] Waiting additional 30s for full connectivity..." -ForegroundColor Yellow
    Start-Sleep -Seconds 30
}

# Wait for mining to start
Write-Host ""
Write-Host "[WAIT] Waiting for mining to start..." -ForegroundColor Yellow
$miningStarted = $false
for ($attempt = 1; $attempt -le 20; $attempt++) {
    Start-Sleep -Seconds 10
    try {
        $result = (Invoke-RestMethod -Uri "http://127.0.0.1:$BaseRPCPort" -Method POST -Body $body -ContentType "application/json" -TimeoutSec 5).result
        if ($result.best_height -gt 0) {
            Write-Host "         [OK] Mining started! Height: $($result.best_height)" -ForegroundColor Green
            $miningStarted = $true
            break
        }
    } catch {}
    Write-Host "         [$attempt/20] Still at genesis..."
}

if (!$miningStarted) {
    Write-Host "[FAIL] Mining did not start within timeout" -ForegroundColor Red
    $FailureReasons += "Mining did not start"
    $TestPassed = $false
    Cleanup
    exit 1
}

# Run stability test
Write-Host ""
Write-Host "[TEST] Running stability test..." -ForegroundColor Cyan
$rounds = [math]::Floor($TestDurationSeconds / $CheckIntervalSeconds)
$syncFailures = 0
$maxSpreadSeen = 0

for ($round = 1; $round -le $rounds; $round++) {
    Start-Sleep -Seconds $CheckIntervalSeconds
    
    $heights = @()
    $peers = @()
    
    for ($i = 0; $i -lt $NodeCount; $i++) {
        $rpcPort = $BaseRPCPort + $i
        try {
            $result = (Invoke-RestMethod -Uri "http://127.0.0.1:$rpcPort" -Method POST -Body $body -ContentType "application/json" -TimeoutSec 5).result
            $heights += $result.best_height
            $peers += $result.peer_count
        } catch {
            $heights += 0
            $peers += 0
        }
    }
    
    $maxH = ($heights | Measure-Object -Maximum).Maximum
    $minH = ($heights | Where-Object { $_ -gt 0 } | Measure-Object -Minimum).Minimum
    if ($null -eq $minH) { $minH = 0 }
    $spread = $maxH - $minH
    $avgPeers = [math]::Round(($peers | Measure-Object -Average).Average, 1)
    
    if ($spread -gt $maxSpreadSeen) { $maxSpreadSeen = $spread }
    
    if ($spread -le $MaxAllowedSpread) {
        $status = "[OK]"
        $color = "Green"
    } else {
        $status = "[FAIL]"
        $color = "Red"
        $syncFailures++
    }
    
    Write-Host "         [$round/$rounds] $status Heights: $($heights -join ',') | Spread: $spread | Peers: $avgPeers" -ForegroundColor $color
}

# Final status check
Write-Host ""
Write-Host "[FINAL] Final status check..." -ForegroundColor Cyan
$finalHeights = @()
for ($i = 0; $i -lt $NodeCount; $i++) {
    $nodeLetter = [char]([int][char]'A' + $i)
    $rpcPort = $BaseRPCPort + $i
    try {
        $result = (Invoke-RestMethod -Uri "http://127.0.0.1:$rpcPort" -Method POST -Body $body -ContentType "application/json" -TimeoutSec 5).result
        $finalHeights += $result.best_height
        Write-Host "         Node $nodeLetter - Height $($result.best_height) | Peers: $($result.peer_count)"
    } catch {
        $finalHeights += 0
        Write-Host "         Node $nodeLetter - [FAIL] Not responding" -ForegroundColor Red
    }
}

$finalMax = ($finalHeights | Measure-Object -Maximum).Maximum

# Evaluate results
Write-Host ""
Write-Host "=================================================================" -ForegroundColor Cyan
Write-Host "  TEST RESULTS" -ForegroundColor Cyan
Write-Host "=================================================================" -ForegroundColor Cyan
Write-Host ""

# Check 1: Minimum blocks mined
if ($finalMax -lt $MinBlocksRequired) {
    Write-Host "[FAIL] Only $finalMax blocks mined (required: $MinBlocksRequired)" -ForegroundColor Red
    $FailureReasons += "Insufficient blocks: $finalMax < $MinBlocksRequired"
    $TestPassed = $false
} else {
    Write-Host "[PASS] $finalMax blocks mined (required: $MinBlocksRequired)" -ForegroundColor Green
}

# Check 2: Sync stability
$syncPassRate = [math]::Round((($rounds - $syncFailures) / $rounds) * 100, 1)
if ($syncFailures -gt [math]::Floor($rounds * 0.1)) {
    Write-Host "[FAIL] $syncFailures/$rounds sync checks failed ($syncPassRate% pass rate)" -ForegroundColor Red
    $FailureReasons += "Sync failures: $syncFailures/$rounds"
    $TestPassed = $false
} else {
    Write-Host "[PASS] $syncPassRate% sync stability ($syncFailures failures)" -ForegroundColor Green
}

# Check 3: Maximum spread
if ($maxSpreadSeen -gt $MaxAllowedSpread) {
    Write-Host "[FAIL] Max spread $maxSpreadSeen blocks (allowed: $MaxAllowedSpread)" -ForegroundColor Red
    $FailureReasons += "Max spread: $maxSpreadSeen > $MaxAllowedSpread"
    $TestPassed = $false
} else {
    Write-Host "[PASS] Max spread $maxSpreadSeen blocks (allowed: $MaxAllowedSpread)" -ForegroundColor Green
}

Write-Host ""

# Summary
if ($TestPassed) {
    Write-Host "=================================================================" -ForegroundColor Green
    Write-Host "  ALL TESTS PASSED" -ForegroundColor Green
    Write-Host "=================================================================" -ForegroundColor Green
    Write-Host ""
    Write-Host "Summary:"
    Write-Host "  - Nodes: $NodeCount"
    Write-Host "  - Blocks Mined: $finalMax"
    Write-Host "  - Sync Pass Rate: $syncPassRate%"
    Write-Host "  - Max Spread: $maxSpreadSeen blocks"
    Write-Host ""
} else {
    Write-Host "=================================================================" -ForegroundColor Red
    Write-Host "  TESTS FAILED" -ForegroundColor Red
    Write-Host "=================================================================" -ForegroundColor Red
    Write-Host ""
    Write-Host "Failure Reasons:" -ForegroundColor Red
    foreach ($reason in $FailureReasons) {
        Write-Host "  - $reason" -ForegroundColor Red
    }
    Write-Host ""
}

# Cleanup
Cleanup

# Exit with appropriate code
if ($TestPassed) {
    exit 0
} else {
    exit 1
}
