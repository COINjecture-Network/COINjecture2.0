@echo off
REM Bootstrap Node for COINjecture Network B
echo ========================================================================
echo Starting COINjecture Network B Bootstrap Node
echo ========================================================================
echo.
echo This node will act as a P2P bootstrap/discovery node for the network.
echo Other nodes can connect to this node using the --bootnodes parameter.
echo.
echo P2P Port: 30333
echo RPC Port: 9933
echo.
echo IMPORTANT: After the node starts, look for the 'Network node PeerId' line.
echo You'll need this PeerId to construct the bootnode multiaddr for other nodes.
echo.
echo Example bootnode format:
echo   /ip4/^<IP_ADDRESS^>/tcp/30333/p2p/^<PEER_ID^>
echo.
echo For local network (same machine):
echo   /ip4/127.0.0.1/tcp/30333/p2p/^<PEER_ID^>
echo.
echo For LAN/remote:
echo   /ip4/^<YOUR_PUBLIC_IP^>/tcp/30333/p2p/^<PEER_ID^>
echo.
echo ========================================================================
echo.

REM Create data directory
if not exist "bootstrap" mkdir bootstrap

REM Run bootstrap node
.\target\release\coinject.exe ^
  --data-dir "bootstrap" ^
  --p2p-addr "/ip4/0.0.0.0/tcp/30333" ^
  --rpc-addr "127.0.0.1:9933" ^
  --difficulty 3 ^
  --block-time 30
