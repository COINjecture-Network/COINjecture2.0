@echo off
title Node3-Bounty
cd /d c:\Users\LEET\COINjecture1337-NETB
echo Starting Node 3 (Bounty)...
.\target\release\coinject.exe --data-dir .\node3-data --p2p-addr /ip4/0.0.0.0/tcp/30335 --rpc-addr 0.0.0.0:9935 --metrics-addr 0.0.0.0:9092 --node-type bounty --mine --bootnodes /ip4/127.0.0.1/tcp/30333 --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose
pause

