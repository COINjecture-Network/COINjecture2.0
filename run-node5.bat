@echo off
title Node5-Oracle
cd /d c:\Users\LEET\COINjecture1337-NETB
echo Starting Node 5 (Oracle)...
.\target\release\coinject.exe --data-dir .\node5-data --p2p-addr /ip4/0.0.0.0/tcp/30337 --rpc-addr 0.0.0.0:9937 --metrics-addr 0.0.0.0:9094 --node-type oracle --mine --bootnodes /ip4/127.0.0.1/tcp/30333 --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose
pause

