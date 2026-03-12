@echo off
title Node6-Light
cd /d c:\Users\LEET\COINjecture1337-NETB
echo ========================================
echo   Node 6 - LIGHT
echo ========================================
echo Light nodes are minimal resource nodes
echo that only track headers and verify proofs.
echo ========================================
echo.
.\target\release\coinject.exe --data-dir .\node6-data --p2p-addr /ip4/0.0.0.0/tcp/30338 --rpc-addr 0.0.0.0:9938 --metrics-addr 0.0.0.0:9095 --node-type light --mine --bootnodes /ip4/127.0.0.1/tcp/30333 --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose
pause

