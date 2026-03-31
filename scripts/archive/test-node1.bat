@echo off
REM Node 1 - Bootnode & Miner
echo Starting Network B Node 1 (Bootnode + Miner)...
echo.

REM Create data directory
if not exist "testnet\node1" mkdir testnet\node1

REM Run node
target\release\coinject.exe ^
  --data-dir testnet/node1 ^
  --p2p-addr "/ip4/0.0.0.0/tcp/30333" ^
  --rpc-addr "127.0.0.1:9933" ^
  --mine ^
  --difficulty 3 ^
  --block-time 30

pause
