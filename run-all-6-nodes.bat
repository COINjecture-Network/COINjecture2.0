@echo off
echo ========================================
echo   COINjecture 6-Node Network Launcher
echo ========================================
echo.
echo Starting ALL 6 node types:
echo   1. Full      (Bootstrap) - Port 30333
echo   2. Archive               - Port 30334
echo   3. Validator             - Port 30335
echo   4. Bounty                - Port 30336
echo   5. Oracle                - Port 30337
echo   6. Light                 - Port 30338
echo.
echo ========================================
cd /d c:\Users\LEET\COINjecture1337-NETB

echo [1/6] Starting Node 1 (Full - Bootstrap)...
start "Node1-Full-Bootstrap" cmd /k "cd /d c:\Users\LEET\COINjecture1337-NETB && echo === NODE 1: FULL (BOOTSTRAP) === && .\target\release\coinject.exe --data-dir .\node1-data --p2p-addr /ip4/0.0.0.0/tcp/30333 --rpc-addr 0.0.0.0:9933 --metrics-addr 0.0.0.0:9090 --mine --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose"

timeout /t 8 /nobreak > nul

echo [2/6] Starting Node 2 (Archive)...
start "Node2-Archive" cmd /k "cd /d c:\Users\LEET\COINjecture1337-NETB && echo === NODE 2: ARCHIVE === && .\target\release\coinject.exe --data-dir .\node2-data --p2p-addr /ip4/0.0.0.0/tcp/30334 --rpc-addr 0.0.0.0:9934 --metrics-addr 0.0.0.0:9091 --node-type archive --mine --bootnodes /ip4/127.0.0.1/tcp/30333 --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose"

timeout /t 3 /nobreak > nul

echo [3/6] Starting Node 3 (Validator)...
start "Node3-Validator" cmd /k "cd /d c:\Users\LEET\COINjecture1337-NETB && echo === NODE 3: VALIDATOR === && .\target\release\coinject.exe --data-dir .\node3-data --p2p-addr /ip4/0.0.0.0/tcp/30335 --rpc-addr 0.0.0.0:9935 --metrics-addr 0.0.0.0:9092 --node-type validator --mine --bootnodes /ip4/127.0.0.1/tcp/30333 --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose"

timeout /t 3 /nobreak > nul

echo [4/6] Starting Node 4 (Bounty)...
start "Node4-Bounty" cmd /k "cd /d c:\Users\LEET\COINjecture1337-NETB && echo === NODE 4: BOUNTY === && .\target\release\coinject.exe --data-dir .\node4-data --p2p-addr /ip4/0.0.0.0/tcp/30336 --rpc-addr 0.0.0.0:9936 --metrics-addr 0.0.0.0:9093 --node-type bounty --mine --bootnodes /ip4/127.0.0.1/tcp/30333 --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose"

timeout /t 3 /nobreak > nul

echo [5/6] Starting Node 5 (Oracle)...
start "Node5-Oracle" cmd /k "cd /d c:\Users\LEET\COINjecture1337-NETB && echo === NODE 5: ORACLE === && .\target\release\coinject.exe --data-dir .\node5-data --p2p-addr /ip4/0.0.0.0/tcp/30337 --rpc-addr 0.0.0.0:9937 --metrics-addr 0.0.0.0:9094 --node-type oracle --mine --bootnodes /ip4/127.0.0.1/tcp/30333 --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose"

timeout /t 3 /nobreak > nul

echo [6/6] Starting Node 6 (Light)...
start "Node6-Light" cmd /k "cd /d c:\Users\LEET\COINjecture1337-NETB && echo === NODE 6: LIGHT === && .\target\release\coinject.exe --data-dir .\node6-data --p2p-addr /ip4/0.0.0.0/tcp/30338 --rpc-addr 0.0.0.0:9938 --metrics-addr 0.0.0.0:9095 --node-type light --mine --bootnodes /ip4/127.0.0.1/tcp/30333 --hf-token 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 --hf-dataset-name COINjecture/v5 --verbose"

echo.
echo ========================================
echo   ALL 6 NODES LAUNCHED!
echo ========================================
echo.
echo Node Types:
echo   1. Full      - Standard full node (bootstrap)
echo   2. Archive   - Stores complete history
echo   3. Validator - Validates blocks/transactions
echo   4. Bounty    - Tracks NP problem bounties
echo   5. Oracle    - External data feeds
echo   6. Light     - Minimal resource node
echo.
echo RPC Endpoints:
echo   Node 1: http://localhost:9933
echo   Node 2: http://localhost:9934
echo   Node 3: http://localhost:9935
echo   Node 4: http://localhost:9936
echo   Node 5: http://localhost:9937
echo   Node 6: http://localhost:9938
echo.
echo HuggingFace: https://huggingface.co/datasets/COINjecture/v5
echo ========================================
pause

