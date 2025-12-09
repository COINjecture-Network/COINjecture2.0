@echo off
REM Node 2 - Miner
echo Starting Network B Node 2 (Miner)...
echo.

REM Create data directory
if not exist "testnet\node2" mkdir testnet\node2

REM Run node
target\release\coinject.exe ^
  --data-dir testnet/node2 ^
  --p2p-addr "/ip4/0.0.0.0/tcp/30334" ^
  --rpc-addr "127.0.0.1:9934" ^
  --mine ^
  --difficulty 3 ^
  --block-time 30

pause
