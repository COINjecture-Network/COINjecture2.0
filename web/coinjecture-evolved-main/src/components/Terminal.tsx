import { useState, useEffect, useRef } from "react";
import { Card } from "@/components/ui/card";
import { Copy, Check } from "lucide-react";

const COMMANDS = [
  "help",
  "mine start",
  "wallet balance",
  "blockchain status",
  "rewards claim",
  "network peers",
  "bounty submit"
];

export const Terminal = () => {
  const [input, setInput] = useState("");
  const [output, setOutput] = useState<string[]>([
    "🚀 COINjecture Web CLI v3.21.0 - $BEANS",
    "Type 'help' to see all available commands",
    "",
    "✅ Demo wallet generated: 0xBEANS0HYM75LZ",
    "🌐 Connected to network - 42 peers online",
    ""
  ]);
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const [copied, setCopied] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const outputRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [output]);

  const handleCommand = (cmd: string) => {
    const trimmedCmd = cmd.trim().toLowerCase();
    setHistory([...history, cmd]);
    setHistoryIndex(-1);

    let response = "";
    
    switch (trimmedCmd) {
      case "help":
        response = `Available commands:
  help              - Show this help message
  mine start        - Start mining BEANS
  wallet balance    - Check your wallet balance
  blockchain status - View blockchain statistics
  rewards claim     - Claim pending rewards
  network peers     - Show connected peers
  bounty submit     - Submit computational bounty
  clear             - Clear terminal`;
        break;
      case "mine start":
        response = "⛏️  Mining started...\n💎 Block #12847 mined! Reward: 2.5 BEANS\n⏱️  Average hashrate: 1.2 TH/s";
        break;
      case "wallet balance":
        response = "💰 Wallet: 0xBEANS0HYM75LZ\n💵 Balance: 127.50 BEANS\n📊 Pending rewards: 2.5 BEANS";
        break;
      case "blockchain status":
        response = "📊 Blockchain Status:\n⛓️  Latest block: #12847\n⚡ Gas price: 38,000-600,000+ (dynamic)\n🔗 Chain ID: 1337\n✅ Status: Healthy";
        break;
      case "rewards claim":
        response = "✅ Claimed 2.5 BEANS\n💰 New balance: 130.00 BEANS\n🎉 Transaction confirmed in block #12848";
        break;
      case "network peers":
        response = "🌐 Connected Peers: 42\n📍 US-East: 12 peers\n📍 EU-West: 18 peers\n📍 Asia: 12 peers";
        break;
      case "bounty submit":
        response = "🎯 Opening bounty submission portal...\n💰 Post computational problems with BEANS rewards\n🌐 Visit: /bounty-submit\n📊 156 active bounties • 2,847 BEANS awarded";
        setTimeout(() => {
          window.location.href = "/bounty-submit";
        }, 1500);
        break;
      case "clear":
        setOutput([]);
        return;
      case "":
        return;
      default:
        response = `Command not found: ${cmd}\nType 'help' for available commands`;
    }
    
    setOutput([...output, `coinjectured$ ${cmd}`, response, ""]);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
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
          <button
            onClick={copyWallet}
            className="flex items-center gap-2 text-xs text-primary hover:text-primary/80 transition-colors"
          >
            <span className="terminal-font">0xBEANS0HYM75LZ</span>
            {copied ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
          </button>
          
          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground">127.50 BEANS</span>
            <div className="w-2 h-2 rounded-full bg-success animate-pulse" />
            <span className="text-xs text-success">Connected</span>
          </div>
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
            className="flex-1 bg-transparent outline-none caret-primary"
            autoFocus
          />
          <span className="animate-pulse">█</span>
        </div>
      </div>
    </Card>
  );
};
