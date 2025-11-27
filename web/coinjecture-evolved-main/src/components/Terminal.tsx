import { useState, useEffect, useRef } from "react";
import { Card } from "@/components/ui/card";
import { Copy, Check, Loader2 } from "lucide-react";
import { useWallet } from "@/contexts/WalletContext";
import { rpcClient } from "@/lib/rpc-client";

const COMMANDS = [
  "help",
  "wallet balance",
  "blockchain status",
  "network peers",
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
