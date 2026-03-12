@echo off
title COINjecture Node 2
cd /d c:\Users\LEET\COINjecture1337-NETB
echo Starting Node 2 (connecting to Node 1)...
.\target\release\coinject.exe --data-dir .\node2-data --p2p-addr /ip4/0.0.0.0/tcp/30334 --rpc-addr 0.0.0.0:9934 --metrics-addr 0.0.0.0:9091 --mine --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWHyA8XJFrQTJpMGuxUw5JqkLk3bEKivnVJYx9ByDD37nF --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose
pause

