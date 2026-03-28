import { useState, useEffect, useRef, useCallback } from "react";
import { Card } from "@/components/ui/card";
import { Copy, Check, Loader2 } from "lucide-react";
import { useWallet } from "@/contexts/WalletContext";
import { rpcClient } from "@/lib/rpc-client";
import { formatBeans, parseBalance } from "@/lib/chain-metrics";
import { createBlock, extractHashHex } from "@/lib/mining";
import { cn } from "@/lib/utils";

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
  "clear",
];

export type WebCliTerminalProps = {
  /** Shorter panel for embedding in Solver Lab */
  compact?: boolean;
  className?: string;
};

export function WebCliTerminal({ compact = false, className }: WebCliTerminalProps) {
  const { selectedAccount, accounts } = useWallet();
  const selectedKeyPair = selectedAccount ? accounts[selectedAccount] : null;
  const [input, setInput] = useState("");
  const [output, setOutput] = useState<string[]>([
    "COINjecture Web CLI v3.21.0 — $BEANS",
    "Type 'help' to see all available commands",
    "",
  ]);
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const [copied, setCopied] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const outputRef = useRef<HTMLDivElement>(null);

  const appendLines = useCallback((lines: string[]) => {
    setOutput((prev) => [...prev, ...lines]);
  }, []);

  useEffect(() => {
    if (outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [output]);

  const handleCommand = async (cmd: string) => {
    const trimmedCmd = cmd.trim().toLowerCase();
    setHistory((h) => [...h, cmd]);
    setHistoryIndex(-1);
    setIsLoading(true);

    let response = "";
    let skipDefaultOutput = false;

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
            response =
              "No wallet connected. Connect a wallet first using the 'Connect Wallet' button.";
          } else {
            try {
              const accountInfo = await rpcClient.getAccountInfo(selectedKeyPair.address);
              response = `Wallet: ${selectedKeyPair.address.slice(0, 16)}...${selectedKeyPair.address.slice(-8)}
Balance: ${accountInfo.balance.toLocaleString()} BEANS
Nonce: ${accountInfo.nonce}`;
            } catch (error: unknown) {
              const msg = error instanceof Error ? error.message : "Unknown error";
              response = `Error fetching wallet balance: ${msg}`;
            }
          }
          break;
        case "blockchain status":
          try {
            const chainInfo = await rpcClient.getChainInfo();
            response = `Blockchain status:
Latest block: #${chainInfo.best_height.toLocaleString()}
Chain ID: ${chainInfo.chain_id}
Peers: ${chainInfo.peer_count}
Status: Healthy`;
          } catch (error: unknown) {
            const msg = error instanceof Error ? error.message : "Unknown error";
            response = `Error fetching blockchain status: ${msg}`;
          }
          break;
        case "network peers":
          try {
            const chainInfo = await rpcClient.getChainInfo();
            response = `Connected peers: ${chainInfo.peer_count}
Network: COINjecture Network B`;
          } catch (error: unknown) {
            const msg = error instanceof Error ? error.message : "Unknown error";
            response = `Error fetching peer count: ${msg}`;
          }
          break;
        case "mine status":
          try {
            const chainInfo = await rpcClient.getChainInfo();
            const latestBlock = await rpcClient.getLatestBlock();

            if (latestBlock) {
              const block = latestBlock;
              const isMiner =
                selectedKeyPair &&
                block.header.miner.toLowerCase() === selectedKeyPair.address.toLowerCase();

              const blockDate = new Date(Number(block.header.timestamp) * 1000);
              response = `Mining status:
Current block: #${block.header.height}
Previous hash: ${block.header.prev_hash.slice(0, 16)}...
Block time: ${blockDate.toLocaleString()}
Miner: ${block.header.miner.slice(0, 16)}...${block.header.miner.slice(-8)}
${isMiner ? "You mined this block." : "Mining in progress…"}
Work score: ${block.header.work_score.toFixed(2)}
Energy: ${block.header.energy_estimate_joules.toFixed(4)} J
Network height: ${chainInfo.best_height}
${chainInfo.peer_count > 0 ? "Connected to network" : "No peers connected"}`;
            } else {
              response = `Mining status:
No blocks found
Network height: ${chainInfo.best_height}`;
            }
          } catch (error: unknown) {
            const msg = error instanceof Error ? error.message : "Unknown error";
            response = `Error fetching mining status: ${msg}`;
          }
          break;
        case "mine stats":
          try {
            const chainInfo = await rpcClient.getChainInfo();
            const latestBlock = await rpcClient.getLatestBlock();

            if (!selectedKeyPair) {
              response = "No wallet connected. Connect a wallet to view mining statistics.";
            } else if (latestBlock) {
              const block = latestBlock;
              const isMiner = block.header.miner.toLowerCase() === selectedKeyPair.address.toLowerCase();

              const accountInfo = await rpcClient.getAccountInfo(selectedKeyPair.address);

              response = `Mining statistics:
Your balance: ${accountInfo.balance.toLocaleString()} BEANS
Your nonce: ${accountInfo.nonce}
${isMiner ? "You are the miner of the latest block." : "Not the miner of latest block."}
Latest block: #${block.header.height}
Work score: ${block.header.work_score.toFixed(2)}
Solution quality: ${(block.header.solution_quality * 100).toFixed(2)}%
Solve time: ${(block.header.solve_time_us / 1000).toFixed(2)}ms
Verify time: ${(block.header.verify_time_us / 1000).toFixed(2)}ms
Network height: ${chainInfo.best_height}
Network peers: ${chainInfo.peer_count}`;
            } else {
              const bal = (await rpcClient.getAccountInfo(selectedKeyPair.address)).balance.toLocaleString();
              response = `Mining statistics:
No blocks found
Your balance: ${bal} BEANS`;
            }
          } catch (error: unknown) {
            const msg = error instanceof Error ? error.message : "Unknown error";
            response = `Error fetching mining stats: ${msg}`;
          }
          break;
        case "mine info":
          try {
            const chainInfo = await rpcClient.getChainInfo();
            const latestBlock = await rpcClient.getLatestBlock();

            if (latestBlock) {
              const block = latestBlock;
              response = `Mining information:
Current difficulty: ${block.header.work_score > 0 ? "Dynamic" : "N/A"}
Work score: ${block.header.work_score.toFixed(2)}
Target block time: ~60s

Latest block:
Height: #${block.header.height}
Miner: ${block.header.miner.slice(0, 16)}...${block.header.miner.slice(-8)}
Complexity: ${block.header.complexity_weight.toFixed(2)}
Energy: ${block.header.energy_estimate_joules.toFixed(4)} J

Network:
Chain ID: ${chainInfo.chain_id}
Best height: ${chainInfo.best_height}
Peers: ${chainInfo.peer_count}

Note: Mining is controlled by the node's --mine flag.`;
            } else {
              response = `Mining information:
No blocks found
Network: ${chainInfo.chain_id}
Peers: ${chainInfo.peer_count}`;
            }
          } catch (error: unknown) {
            const msg = error instanceof Error ? error.message : "Unknown error";
            response = `Error fetching mining info: ${msg}`;
          }
          break;
        case "mine submit":
          skipDefaultOutput = true;
          if (!selectedKeyPair) {
            appendLines([`coinjectured$ ${cmd}`, "No wallet connected. Connect a wallet first to submit mined blocks.", ""]);
          } else {
            try {
              const chainInfo = await rpcClient.getChainInfo();
              const latestBlock = await rpcClient.getLatestBlock();

              if (!latestBlock || !chainInfo.best_hash) {
                appendLines([`coinjectured$ ${cmd}`, "No blocks found. Cannot submit block without chain state.", ""]);
              } else {
                const nextHeight = chainInfo.best_height + 1;

                appendLines([
                  `coinjectured$ ${cmd}`,
                  `Starting block mining…
Latest block: #${chainInfo.best_height}
Next height: #${nextHeight}
Your address: ${selectedKeyPair.address.slice(0, 16)}...${selectedKeyPair.address.slice(-8)}`,
                  "",
                ]);

                try {
                  const prevHashHex = extractHashHex(chainInfo.best_hash);

                  const block = await createBlock(
                    prevHashHex,
                    nextHeight,
                    selectedKeyPair.address,
                    [],
                    10,
                    2
                  );

                  if (!block) {
                    appendLines(["Mining failed. Could not create block.", ""]);
                  } else {
                    const finalChainInfo = await rpcClient.getChainInfo();

                    if (finalChainInfo.best_height >= block.header.height) {
                      appendLines([
                        `Chain advanced during mining (current: #${finalChainInfo.best_height}, mined: #${block.header.height}). Try again.`,
                        "",
                      ]);
                    } else if (
                      finalChainInfo.best_hash &&
                      extractHashHex(finalChainInfo.best_hash) !== prevHashHex
                    ) {
                      appendLines(["Chain advanced during mining. Best block hash changed; try again.", ""]);
                    } else {
                      try {
                        const blockHash = await rpcClient.submitBlock(block);

                        let accountInfo;
                        let attempts = 0;
                        const maxAttempts = 5;

                        while (attempts < maxAttempts) {
                          await new Promise((resolve) => setTimeout(resolve, 1000));
                          try {
                            accountInfo = await rpcClient.getAccountInfo(selectedKeyPair.address);
                            break;
                          } catch {
                            attempts++;
                          }
                        }

                        const rewardB = parseBalance(block.coinbase?.reward);
                        const rewardStr = rewardB !== null ? formatBeans(rewardB) : "0";
                        const currentBalance = accountInfo?.balance || 0;

                        appendLines([
                          `Block mined and submitted successfully.

Block:
Height: #${block.header.height}
Hash: ${blockHash.slice(0, 16)}…
Nonce: ${block.header.nonce}
Work score: ${block.header.work_score.toFixed(2)}
Solve time: ${(block.header.solve_time_us / 1000).toFixed(2)}ms
Energy: ${block.header.energy_estimate_joules.toFixed(4)} J
Reward: ${rewardStr} BEANS

Your balance: ${currentBalance.toLocaleString()} BEANS

Your block is being processed by the network.`,
                          "",
                        ]);
                      } catch (error: unknown) {
                        const errorMsg = error instanceof Error ? error.message : "";
                        if (
                          errorMsg.includes("previous hash") ||
                          errorMsg.includes("Invalid previous hash") ||
                          errorMsg.includes("height")
                        ) {
                          appendLines(["Chain advanced during mining. Try again.", ""]);
                        } else {
                          appendLines([`Block submission failed: ${errorMsg || "Unknown error"}`, ""]);
                        }
                      }
                    }
                  }
                } catch (error: unknown) {
                  const msg = error instanceof Error ? error.message : "Unknown error";
                  appendLines([`Mining error: ${msg}`, ""]);
                }
              }
            } catch (error: unknown) {
              const msg = error instanceof Error ? error.message : "Unknown error";
              appendLines([`coinjectured$ ${cmd}`, `Error: ${msg}`, ""]);
            }
          }
          break;
        case "bounty submit":
          response = "Opening bounty submission…";
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
          response = `Command not found: ${cmd}
Type 'help' for available commands`;
      }
    } catch (error: unknown) {
      const msg = error instanceof Error ? error.message : "Unknown error";
      response = `Error: ${msg}`;
    } finally {
      setIsLoading(false);
    }

    if (!skipDefaultOutput) {
      appendLines([`coinjectured$ ${cmd}`, response, ""]);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !isLoading) {
      void handleCommand(input);
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
      const matches = COMMANDS.filter((c) => c.startsWith(input));
      if (matches.length === 1) {
        setInput(matches[0]);
      }
    }
  };

  return (
    <Card className={cn("glass-effect p-0 overflow-hidden border-border/60", className)}>
      <div className="bg-muted/40 border-b border-border px-4 py-2.5 flex items-center justify-between gap-2 flex-wrap">
        <div className="flex items-center gap-2 min-w-0">
          <div className="flex gap-1.5 shrink-0">
            <div className="w-2.5 h-2.5 rounded-full bg-destructive/90" />
            <div className="w-2.5 h-2.5 rounded-full bg-warning/90" />
            <div className="w-2.5 h-2.5 rounded-full bg-success/90" />
          </div>
          <span className="text-xs sm:text-sm text-foreground/90 font-mono truncate">
            COINjecture Web CLI ($BEANS)
          </span>
        </div>

        <div className="flex items-center gap-3 text-xs">
          {selectedKeyPair && (
            <button
              type="button"
              onClick={() => {
                void navigator.clipboard.writeText(selectedKeyPair.address);
                setCopied(true);
                setTimeout(() => setCopied(false), 2000);
              }}
              className="flex items-center gap-1.5 text-primary hover:text-primary/80 transition-colors max-w-[200px]"
            >
              <span className="font-mono truncate">
                {selectedKeyPair.address.slice(0, 12)}…{selectedKeyPair.address.slice(-6)}
              </span>
              {copied ? <Check className="h-3 w-3 shrink-0" /> : <Copy className="h-3 w-3 shrink-0" />}
            </button>
          )}

          {selectedKeyPair ? (
            <div className="flex items-center gap-1.5">
              <div className="w-1.5 h-1.5 rounded-full bg-success animate-pulse" />
              <span className="text-muted-foreground hidden sm:inline">Wallet</span>
            </div>
          ) : (
            <div className="flex items-center gap-1.5">
              <div className="w-1.5 h-1.5 rounded-full bg-muted-foreground/50" />
              <span className="text-muted-foreground">No wallet</span>
            </div>
          )}
        </div>
      </div>

      <div
        ref={outputRef}
        className={cn(
          "bg-terminal-bg p-4 overflow-y-auto terminal-font text-sm text-terminal-text",
          compact ? "min-h-[200px] max-h-[min(50vh,420px)]" : "min-h-[320px] h-[min(50vh,420px)]"
        )}
        onClick={() => inputRef.current?.focus()}
      >
        {output.map((line, i) => (
          <div key={i} className="whitespace-pre-wrap break-words">
            {line}
          </div>
        ))}

        <div className="flex items-center gap-2 mt-1">
          <span className="text-primary shrink-0">coinjectured$</span>
          <input
            ref={inputRef}
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            disabled={isLoading}
            className="flex-1 min-w-0 bg-transparent outline-none caret-primary disabled:opacity-50"
            autoFocus
            aria-label="CLI command input"
          />
          {isLoading ? (
            <Loader2 className="h-4 w-4 animate-spin text-primary shrink-0" />
          ) : (
            <span className="animate-pulse opacity-60">▍</span>
          )}
        </div>
      </div>
    </Card>
  );
}
