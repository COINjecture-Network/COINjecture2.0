import { useState, useEffect, useRef } from "react";
import { Card } from "@/components/ui/card";
import { Copy, Check, Loader2 } from "lucide-react";
import { useWallet } from "@/contexts/WalletContext";
import { rpcClient } from "@/lib/rpc-client";
import { createBlock, extractHashHex } from "@/lib/mining";

const COMMANDS = [
  "help",
  "wallet balance",
  "blockchain status",
  "network peers",
  "mine status",
  "mine stats",
  "mine info",
  "mine submit",
  "bounty submit",
  "clear"
];

export const Terminal = () => {
  const { selectedAccount, accounts } = useWallet();
  const selectedKeyPair = selectedAccount ? accounts[selectedAccount] : null;
  const [input, setInput] = useState("");
  const [output, setOutput] = useState<string[]>([
    "🚀 COINjecture Web CLI v3.21.0 - $BEANS",
    "Type 'help' to see all available commands",
    "",
  ]);
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const [copied, setCopied] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const outputRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [output]);

  const handleCommand = async (cmd: string) => {
    const trimmedCmd = cmd.trim().toLowerCase();
    setHistory([...history, cmd]);
    setHistoryIndex(-1);
    setIsLoading(true);

    let response = "";
    
    try {
      switch (trimmedCmd) {
        case "help":
          response = `Available commands:
  help              - Show this help message
  wallet balance    - Check your wallet balance
  blockchain status - View blockchain statistics
  network peers     - Show connected peers
  mine status       - Show mining status and current block info
  mine stats        - Show mining statistics and rewards
  mine info         - Show mining configuration
  mine submit       - Submit a mined block (requires block data)
  bounty submit     - Submit computational bounty
  clear             - Clear terminal`;
          break;
        case "wallet balance":
          if (!selectedKeyPair) {
            response = "❌ No wallet connected\n💡 Connect a wallet first using the 'Connect Wallet' button";
          } else {
            try {
              const accountInfo = await rpcClient.getAccountInfo(selectedKeyPair.address);
              response = `💰 Wallet: ${selectedKeyPair.address.slice(0, 16)}...${selectedKeyPair.address.slice(-8)}\n💵 Balance: ${accountInfo.balance.toLocaleString()} BEANS\n📊 Nonce: ${accountInfo.nonce}`;
            } catch (error: any) {
              response = `❌ Error fetching wallet balance: ${error.message || 'Unknown error'}`;
            }
          }
          break;
        case "blockchain status":
          try {
            const chainInfo = await rpcClient.getChainInfo();
            response = `📊 Blockchain Status:\n⛓️  Latest block: #${chainInfo.best_height.toLocaleString()}\n🔗 Chain ID: ${chainInfo.chain_id}\n🌐 Peers: ${chainInfo.peer_count}\n✅ Status: Healthy`;
          } catch (error: any) {
            response = `❌ Error fetching blockchain status: ${error.message || 'Unknown error'}`;
          }
          break;
        case "network peers":
          try {
            const chainInfo = await rpcClient.getChainInfo();
            response = `🌐 Connected Peers: ${chainInfo.peer_count}\n📍 Network: COINjecture Network B`;
          } catch (error: any) {
            response = `❌ Error fetching peer count: ${error.message || 'Unknown error'}`;
          }
          break;
        case "mine status":
          try {
            const chainInfo = await rpcClient.getChainInfo();
            const latestBlock = await rpcClient.getLatestBlock();
            
            if (latestBlock) {
              const block = latestBlock;
              const isMiner = selectedKeyPair && 
                block.header.miner.toLowerCase() === selectedKeyPair.address.toLowerCase();
              
              // Timestamp is in seconds, convert to milliseconds for Date
              const blockDate = new Date(Number(block.header.timestamp) * 1000);
              response = `⛏️  Mining Status:
📊 Current Block: #${block.header.height}
🔗 Previous Hash: ${block.header.prev_hash.slice(0, 16)}...
⏰ Block Time: ${blockDate.toLocaleString()}
💎 Miner: ${block.header.miner.slice(0, 16)}...${block.header.miner.slice(-8)}
${isMiner ? '✅ You mined this block!' : '⏳ Mining in progress...'}
⚡ Work Score: ${block.header.work_score.toFixed(2)}
🔋 Energy: ${block.header.energy_estimate_joules.toFixed(4)} J
🌐 Network Height: ${chainInfo.best_height}
${chainInfo.peer_count > 0 ? '✅ Connected to network' : '⚠️  No peers connected'}`;
            } else {
              response = `⛏️  Mining Status:\n⚠️  No blocks found\n🌐 Network Height: ${chainInfo.best_height}`;
            }
          } catch (error: any) {
            response = `❌ Error fetching mining status: ${error.message || 'Unknown error'}`;
          }
          break;
        case "mine stats":
          try {
            const chainInfo = await rpcClient.getChainInfo();
            const latestBlock = await rpcClient.getLatestBlock();
            
            if (!selectedKeyPair) {
              response = "❌ No wallet connected\n💡 Connect a wallet to view mining statistics";
            } else if (latestBlock) {
              const block = latestBlock;
              const isMiner = block.header.miner.toLowerCase() === selectedKeyPair.address.toLowerCase();
              
              // Get account info to see rewards
              const accountInfo = await rpcClient.getAccountInfo(selectedKeyPair.address);
              
              response = `📊 Mining Statistics:
💰 Your Balance: ${accountInfo.balance.toLocaleString()} BEANS
📈 Your Nonce: ${accountInfo.nonce}
${isMiner ? '✅ You are the miner of the latest block!' : '⏳ Not the miner of latest block'}
📦 Latest Block: #${block.header.height}
⚡ Work Score: ${block.header.work_score.toFixed(2)}
🎯 Solution Quality: ${(block.header.solution_quality * 100).toFixed(2)}%
⏱️  Solve Time: ${(block.header.solve_time_us / 1000).toFixed(2)}ms
🔍 Verify Time: ${(block.header.verify_time_us / 1000).toFixed(2)}ms
🌐 Network Height: ${chainInfo.best_height}
👥 Network Peers: ${chainInfo.peer_count}`;
            } else {
              response = `📊 Mining Statistics:\n⚠️  No blocks found\n💰 Your Balance: ${(await rpcClient.getAccountInfo(selectedKeyPair.address)).balance.toLocaleString()} BEANS`;
            }
          } catch (error: any) {
            response = `❌ Error fetching mining stats: ${error.message || 'Unknown error'}`;
          }
          break;
        case "mine info":
          try {
            const chainInfo = await rpcClient.getChainInfo();
            const latestBlock = await rpcClient.getLatestBlock();
            
            if (latestBlock) {
              const block = latestBlock;
              response = `⛏️  Mining Information:
🔧 Configuration:
  📊 Current Difficulty: ${block.header.work_score > 0 ? 'Dynamic' : 'N/A'}
  ⚡ Work Score: ${block.header.work_score.toFixed(2)}
  🎯 Target Block Time: ~60 seconds
  
📦 Latest Block Details:
  Height: #${block.header.height}
  Miner: ${block.header.miner.slice(0, 16)}...${block.header.miner.slice(-8)}
  Complexity: ${block.header.complexity_weight.toFixed(2)}
  Energy: ${block.header.energy_estimate_joules.toFixed(4)} J
  
🌐 Network:
  Chain ID: ${chainInfo.chain_id}
  Best Height: ${chainInfo.best_height}
  Peers: ${chainInfo.peer_count}
  
💡 Note: Mining is controlled by the node's --mine flag.
   Connect your wallet to see if you're mining blocks.`;
            } else {
              response = `⛏️  Mining Information:\n⚠️  No blocks found\n🌐 Network: ${chainInfo.chain_id}\n👥 Peers: ${chainInfo.peer_count}`;
            }
          } catch (error: any) {
            response = `❌ Error fetching mining info: ${error.message || 'Unknown error'}`;
          }
          break;
        case "mine submit":
          if (!selectedKeyPair) {
            response = "❌ No wallet connected\n💡 Connect a wallet first to submit mined blocks";
          } else {
            try {
              // Fetch chain info to get the current best block hash
              const chainInfo = await rpcClient.getChainInfo();
              const latestBlock = await rpcClient.getLatestBlock();
              
              if (!latestBlock || !chainInfo.best_hash) {
                response = "❌ No blocks found\n⚠️  Cannot submit block without chain state";
              } else {
                const nextHeight = chainInfo.best_height + 1;
                
                response = `⛏️  Starting block mining...

📊 Current Chain State:
  Latest Block: #${chainInfo.best_height}
  Next Height: #${nextHeight}
  Your Address: ${selectedKeyPair.address.slice(0, 16)}...${selectedKeyPair.address.slice(-8)}

⛏️  Mining block #${nextHeight}...`;
                
                setOutput([...output, `coinjectured$ ${cmd}`, response, ""]);
                
                // Start mining process
                try {
                  // Use best_hash from chain info as prev_hash for the new block
                  // best_hash is the hash of the current best block
                  const prevHashHex = extractHashHex(chainInfo.best_hash);
                  
                  const block = await createBlock(
                    prevHashHex,
                    nextHeight,
                    selectedKeyPair.address,
                    [],
                    10, // problem size
                    2   // difficulty
                  );
                  
                  if (!block) {
                    response = `❌ Mining failed\n⚠️  Could not create block`;
                  } else {
                    // Re-fetch chain state right before submission to ensure prev_hash is still correct
                    const finalChainInfo = await rpcClient.getChainInfo();
                    
                    // Check if chain advanced during mining
                    if (finalChainInfo.best_height >= block.header.height) {
                      response = `⚠️  Chain advanced during mining (current: #${finalChainInfo.best_height}, mined: #${block.header.height})\n💡 Please try again`;
                    } else if (finalChainInfo.best_hash && extractHashHex(finalChainInfo.best_hash) !== prevHashHex) {
                      response = `⚠️  Chain advanced during mining\n💡 Best block hash changed, please try again`;
                    } else {
                      // Submit block
                      let blockHash = '';
                      try {
                        blockHash = await rpcClient.submitBlock(block);
                        
                        // Wait a moment for block to be processed, then check balance multiple times
                        let accountInfo;
                        let attempts = 0;
                        const maxAttempts = 5;
                        
                        while (attempts < maxAttempts) {
                          await new Promise(resolve => setTimeout(resolve, 1000));
                          try {
                            accountInfo = await rpcClient.getAccountInfo(selectedKeyPair.address);
                            // If we got balance info, break
                            break;
                          } catch (error: any) {
                            attempts++;
                            if (attempts >= maxAttempts) {
                              console.error('Failed to fetch balance after block submission:', error);
                            }
                          }
                        }
                        
                        const reward = block.coinbase.reward || 0;
                        const currentBalance = accountInfo?.balance || 0;
                        
                        response = `✅ Block mined and submitted successfully!

📦 Block Details:
  Height: #${block.header.height}
  Hash: ${blockHash.slice(0, 16)}...
  Nonce: ${block.header.nonce}
  Work Score: ${block.header.work_score.toFixed(2)}
  Solve Time: ${(block.header.solve_time_us / 1000).toFixed(2)}ms
  Energy: ${block.header.energy_estimate_joules.toFixed(4)} J
  Reward: ${reward.toLocaleString()} BEANS

💰 Your Balance: ${currentBalance.toLocaleString()} BEANS

🎉 Your block is being processed by the network!
💡 Note: If the block is accepted as the new best block, you'll receive the reward.
   If server already mined that height, your block is stored as a fork and reward may be applied later.`;
                      } catch (error: any) {
                        // Check if error is due to invalid prev_hash
                        const errorMsg = error.message || '';
                        if (errorMsg.includes('previous hash') || errorMsg.includes('Invalid previous hash') || errorMsg.includes('height')) {
                          response = `⚠️  Chain advanced during mining\n💡 The chain moved while you were mining. Please try again.`;
                        } else {
                          response = `❌ Block submission failed: ${errorMsg}`;
                        }
                      }
                    }
                  }
                } catch (error: any) {
                  response = `❌ Mining error: ${error.message || 'Unknown error'}`;
                }
              }
            } catch (error: any) {
              response = `❌ Error: ${error.message || 'Unknown error'}`;
            }
          }
          break;
        case "bounty submit":
          response = "🎯 Opening bounty submission portal...\n💰 Post computational problems with BEANS rewards\n🌐 Visit: /bounty-submit";
          setTimeout(() => {
            window.location.href = "/bounty-submit";
          }, 1500);
          break;
        case "clear":
          setOutput([]);
          setIsLoading(false);
          return;
        case "":
          setIsLoading(false);
          return;
        default:
          response = `Command not found: ${cmd}\nType 'help' for available commands`;
      }
    } catch (error: any) {
      response = `❌ Error: ${error.message || 'Unknown error'}`;
    } finally {
      setIsLoading(false);
    }
    
    setOutput([...output, `coinjectured$ ${cmd}`, response, ""]);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !isLoading) {
      handleCommand(input);
      setInput("");
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      if (history.length > 0) {
        const newIndex = historyIndex + 1;
        if (newIndex < history.length) {
          setHistoryIndex(newIndex);
          setInput(history[history.length - 1 - newIndex]);
        }
      }
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      const newIndex = historyIndex - 1;
      if (newIndex >= 0) {
        setHistoryIndex(newIndex);
        setInput(history[history.length - 1 - newIndex]);
      } else {
        setHistoryIndex(-1);
        setInput("");
      }
    } else if (e.key === "Tab") {
      e.preventDefault();
      const matches = COMMANDS.filter(cmd => cmd.startsWith(input));
      if (matches.length === 1) {
        setInput(matches[0]);
      }
    }
  };

  const copyWallet = () => {
    navigator.clipboard.writeText("0xBEANS0HYM75LZ");
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <Card className="glass-effect p-0 overflow-hidden glow-primary">
      {/* Terminal Header */}
      <div className="bg-terminal-bg border-b border-border/50 px-6 py-3 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex gap-2">
            <div className="w-3 h-3 rounded-full bg-destructive" />
            <div className="w-3 h-3 rounded-full bg-warning" />
            <div className="w-3 h-3 rounded-full bg-success" />
          </div>
          <span className="text-sm text-terminal-text terminal-font">COINjecture Web CLI ($BEANS)</span>
        </div>
        
        <div className="flex items-center gap-4">
          {selectedKeyPair && (
            <button
              onClick={() => {
                navigator.clipboard.writeText(selectedKeyPair.address);
                setCopied(true);
                setTimeout(() => setCopied(false), 2000);
              }}
              className="flex items-center gap-2 text-xs text-primary hover:text-primary/80 transition-colors"
            >
              <span className="terminal-font">{selectedKeyPair.address.slice(0, 16)}...{selectedKeyPair.address.slice(-8)}</span>
              {copied ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
            </button>
          )}
          
          {selectedKeyPair ? (
            <div className="flex items-center gap-2">
              <div className="w-2 h-2 rounded-full bg-success animate-pulse" />
              <span className="text-xs text-success">Wallet Connected</span>
            </div>
          ) : (
            <div className="flex items-center gap-2">
              <div className="w-2 h-2 rounded-full bg-muted" />
              <span className="text-xs text-muted-foreground">No Wallet</span>
            </div>
          )}
        </div>
      </div>

      {/* Terminal Output */}
      <div 
        ref={outputRef}
        className="bg-terminal-bg p-6 h-96 overflow-y-auto terminal-font text-sm"
        onClick={() => inputRef.current?.focus()}
      >
        {output.map((line, i) => (
          <div key={i} className="text-terminal-text whitespace-pre-wrap">
            {line}
          </div>
        ))}
        
        {/* Input Line */}
        <div className="flex items-center gap-2 text-terminal-text">
          <span>coinjectured$</span>
          <input
            ref={inputRef}
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            disabled={isLoading}
            className="flex-1 bg-transparent outline-none caret-primary disabled:opacity-50"
            autoFocus
          />
          {isLoading ? (
            <Loader2 className="h-4 w-4 animate-spin text-primary" />
          ) : (
            <span className="animate-pulse">█</span>
          )}
        </div>
      </div>
    </Card>
  );
};
