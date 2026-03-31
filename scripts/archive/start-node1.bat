@echo off
title COINjecture Node 1 - Bootstrap
cd /d c:\Users\LEET\COINjecture1337-NETB
echo Starting Node 1 (Bootstrap)...
.\target\release\coinject.exe --data-dir .\node1-data --p2p-addr /ip4/0.0.0.0/tcp/30333 --rpc-addr 0.0.0.0:9933 --metrics-addr 0.0.0.0:9090 --mine --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose
pause

