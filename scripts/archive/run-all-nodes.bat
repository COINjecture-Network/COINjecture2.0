@echo off
echo Starting 4-Node COINjecture Network...
echo.

cd /d c:\Users\LEET\COINjecture1337-NETB

echo [1/4] Starting Node 1 (Full - Bootstrap)...
start "Node1-Full" cmd /k "cd /d c:\Users\LEET\COINjecture1337-NETB && .\target\release\coinject.exe --data-dir .\node1-data --p2p-addr /ip4/0.0.0.0/tcp/30333 --rpc-addr 0.0.0.0:9933 --metrics-addr 0.0.0.0:9090 --mine --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose"
timeout /t 5 /nobreak > nul

echo [2/4] Starting Node 2 (Archive)...
start "Node2-Archive" cmd /k "cd /d c:\Users\LEET\COINjecture1337-NETB && .\target\release\coinject.exe --data-dir .\node2-data --p2p-addr /ip4/0.0.0.0/tcp/30334 --rpc-addr 0.0.0.0:9934 --metrics-addr 0.0.0.0:9091 --node-type archive --mine --bootnodes /ip4/127.0.0.1/tcp/30333 --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose"
timeout /t 3 /nobreak > nul

echo [3/4] Starting Node 3 (Bounty)...
start "Node3-Bounty" cmd /k "cd /d c:\Users\LEET\COINjecture1337-NETB && .\target\release\coinject.exe --data-dir .\node3-data --p2p-addr /ip4/0.0.0.0/tcp/30335 --rpc-addr 0.0.0.0:9935 --metrics-addr 0.0.0.0:9092 --node-type bounty --mine --bootnodes /ip4/127.0.0.1/tcp/30333 --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose"
timeout /t 3 /nobreak > nul

echo [4/4] Starting Node 4 (Validator)...
start "Node4-Validator" cmd /k "cd /d c:\Users\LEET\COINjecture1337-NETB && .\target\release\coinject.exe --data-dir .\node4-data --p2p-addr /ip4/0.0.0.0/tcp/30336 --rpc-addr 0.0.0.0:9936 --metrics-addr 0.0.0.0:9093 --node-type validator --mine --bootnodes /ip4/127.0.0.1/tcp/30333 --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose"

echo.
echo ========================================
echo All 4 nodes launched!
echo ========================================
echo Node 1: Full (Bootstrap) - Port 30333
echo Node 2: Archive          - Port 30334
echo Node 3: Bounty           - Port 30335
echo Node 4: Validator        - Port 30336
echo ========================================
echo.
pause

