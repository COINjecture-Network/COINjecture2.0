@echo off
title COINjecture Node 3
cd /d c:\Users\LEET\COINjecture1337-NETB
echo Starting Node 3 (connecting to Node 1)...
.\target\release\coinject.exe --data-dir .\node3-data --p2p-addr /ip4/0.0.0.0/tcp/30335 --rpc-addr 0.0.0.0:9935 --metrics-addr 0.0.0.0:9092 --mine --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWHyA8XJFrQTJpMGuxUw5JqkLk3bEKivnVJYx9ByDD37nF --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose
pause

