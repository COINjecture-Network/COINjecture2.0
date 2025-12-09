@echo off
title Node2-Archive
cd /d c:\Users\LEET\COINjecture1337-NETB
echo Starting Node 2 (Archive)...
.\target\release\coinject.exe --data-dir .\node2-data --p2p-addr /ip4/0.0.0.0/tcp/30334 --rpc-addr 0.0.0.0:9934 --metrics-addr 0.0.0.0:9091 --node-type archive --mine --bootnodes /ip4/127.0.0.1/tcp/30333 --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose
pause

