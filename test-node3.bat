@echo off
REM Node 3 - Validator Only
echo Starting Network B Node 3 (Validator)...
echo.

REM Create data directory
if not exist "testnet\node3" mkdir testnet\node3

REM Run node
target\release\coinject.exe ^
  --data-dir testnet/node3 ^
  --p2p-addr "/ip4/0.0.0.0/tcp/30335" ^
  --rpc-addr "127.0.0.1:9935" ^
  --difficulty 3 ^
  --block-time 30

pause
