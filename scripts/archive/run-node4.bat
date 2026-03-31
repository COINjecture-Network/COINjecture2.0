@echo off
title Node4-Validator
cd /d c:\Users\LEET\COINjecture1337-NETB
echo Starting Node 4 (Validator)...
.\target\release\coinject.exe --data-dir .\node4-data --p2p-addr /ip4/0.0.0.0/tcp/30336 --rpc-addr 0.0.0.0:9936 --metrics-addr 0.0.0.0:9093 --node-type validator --mine --bootnodes /ip4/127.0.0.1/tcp/30333 --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose
pause

