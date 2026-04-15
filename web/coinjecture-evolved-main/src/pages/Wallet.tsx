import { Navigation } from "@/components/Navigation";
import { Footer } from "@/components/Footer";
import { BrandLogo } from "@/components/BrandLogo";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { useWallet } from "@/contexts/WalletContext";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { rpcClient, type ProblemInfo } from "@/lib/rpc-client";
import { isMarketplaceListingOpen } from "@/lib/marketplace-status";
import { getWalletTransactions, type WalletActivityItem } from "@/lib/api/client";
import { Wallet, Plus, Upload, Send, Copy, Trash2, Eye, EyeOff, Check, ChevronDown } from "lucide-react";
import { useState, type ReactNode } from "react";
import { createSignedTransferTransaction } from "@/lib/wallet-crypto";
import {
  displayBeansToAtoms,
  formatBeans,
  REWARD_FIXED_POINT_SCALE,
} from "@/lib/chain-metrics";
import { toast } from "sonner";
import { Link } from "react-router-dom";

/** Miner tip in ledger atoms (= 1000 display BEANS at S = 10^12). */
const TRANSFER_PRIORITY_TIP_ATOMS = 1000n * REWARD_FIXED_POINT_SCALE;

function isPoolFeeTooLowMessage(message: string): boolean {
  const m = message.toLowerCase();
  return (
    m.includes("fee below minimum") ||
    m.includes("feetoolow") ||
    m.includes("transaction fee below")
  );
}

export default function WalletPage() {
  const { accounts, selectedAccount, setSelectedAccount, createAccount, importAccount, deleteAccount } = useWallet();
  const [showNewAccount, setShowNewAccount] = useState(false);
  const [showImport, setShowImport] = useState(false);
  const [showSend, setShowSend] = useState(false);
  const [newAccountName, setNewAccountName] = useState("");
  const [importAccountName, setImportAccountName] = useState("");
  const [importPrivateKey, setImportPrivateKey] = useState("");
  const [copied, setCopied] = useState<string | null>(null);

  const selectedKeyPair = selectedAccount ? accounts[selectedAccount] : null;

  const handleCreateAccount = () => {
    if (!newAccountName.trim()) {
      toast.error("Please enter an account name");
      return;
    }
    try {
      createAccount(newAccountName.trim());
      setSelectedAccount(newAccountName.trim());
      setNewAccountName("");
      setShowNewAccount(false);
      toast.success("Account created successfully");
    } catch (error: any) {
      toast.error(error.message || "Failed to create account");
    }
  };

  const handleImportAccount = () => {
    if (!importAccountName.trim()) {
      toast.error("Please enter an account name");
      return;
    }
    if (!importPrivateKey.trim()) {
      toast.error("Please enter a private key");
      return;
    }
    try {
      importAccount(importAccountName.trim(), importPrivateKey.trim());
      setSelectedAccount(importAccountName.trim());
      setImportAccountName("");
      setImportPrivateKey("");
      setShowImport(false);
      toast.success("Account imported successfully");
    } catch (error: any) {
      toast.error(error.message || "Failed to import account");
    }
  };

  const copyToClipboard = (text: string, label: string) => {
    navigator.clipboard.writeText(text);
    setCopied(label);
    setTimeout(() => setCopied(null), 2000);
    toast.success("Copied to clipboard");
  };

  return (
    <div className="min-h-screen">
      <Navigation />
      {/* Testnet Warning Banner */}
      <div className="bg-yellow-500/10 border border-yellow-500/30 text-yellow-200 px-6 py-3 text-center text-sm mt-16">
        <span className="font-bold">⚠️ TESTNET ONLY</span> — This wallet is for demonstration purposes only.{" "}
        Private keys are stored in your browser's localStorage and are <span className="font-bold">not secure</span>.{" "}
        Do not use with real funds.
      </div>
      <div className="pt-8 pb-20 container mx-auto px-6">
        <div className="max-w-6xl mx-auto">
          <div className="market-surface-strong p-6 md:p-8 mb-8">
            <div className="grid gap-5 lg:grid-cols-[1.1fr_0.9fr] lg:items-center">
              <div className="flex flex-col sm:flex-row sm:items-start gap-5">
                <BrandLogo size="lg" className="hidden sm:inline-flex" />
                <div className="min-w-0 flex-1">
                <div className="signal-kicker">Miner activation</div>
                <h1 className="text-4xl font-bold mt-2">Wallet</h1>
                <p className="text-muted-foreground mt-3 max-w-2xl">
                  Create or import an account, confirm your balance, and move directly into Solver Lab when you are ready to compete for the next block reward.
                </p>
                <div className="provenance-row mt-4">
                  <span>Create wallet</span>
                  <span>•</span>
                  <span>Select account</span>
                  <span>•</span>
                  <span>Open Solver Lab</span>
                  <span>•</span>
                  <span>Submit block</span>
                </div>
                </div>
              </div>
              <div className="grid gap-3 sm:grid-cols-2">
                <div className="signal-card">
                  <div className="signal-kicker">Current status</div>
                  <div className="mt-2 font-semibold">
                    {selectedKeyPair ? `Ready as ${selectedAccount}` : "No active wallet selected"}
                  </div>
                </div>
                <div className="signal-card">
                  <div className="signal-kicker">Next best action</div>
                  <div className="mt-2 font-semibold">
                    {selectedKeyPair ? "Open Solver Lab and sync a live mining instance." : "Create or import an account to start earning."}
                  </div>
                </div>
                <div className="sm:col-span-2 flex flex-col sm:flex-row gap-3">
                  <Button asChild className="sm:flex-1">
                    <Link to="/solver-lab">Start Mining</Link>
                  </Button>
                  <Button asChild variant="outline" className="sm:flex-1">
                    <Link to="/marketplace">View Live Market</Link>
                  </Button>
                </div>
              </div>
            </div>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            {/* Left: Account List */}
            <Card className="p-6">
              <div className="flex items-center justify-between mb-6">
                <h2 className="text-2xl font-semibold">My Accounts</h2>
                <Dialog open={showNewAccount} onOpenChange={setShowNewAccount}>
                  <DialogTrigger asChild>
                    <Button size="sm">
                      <Plus className="h-4 w-4 mr-2" />
                      New
                    </Button>
                  </DialogTrigger>
                  <DialogContent>
                    <DialogHeader>
                      <DialogTitle>Create New Account</DialogTitle>
                      <DialogDescription>
                        Generate a new Ed25519 keypair for your wallet
                        <span className="block text-yellow-400 text-xs mt-1">⚠️ Testnet only — keys stored in localStorage</span>
                      </DialogDescription>
                    </DialogHeader>
                    <div className="space-y-4">
                      <div>
                        <Label htmlFor="account-name">Account Name</Label>
                        <Input
                          id="account-name"
                          value={newAccountName}
                          onChange={(e) => setNewAccountName(e.target.value)}
                          placeholder="e.g., My Wallet"
                          autoFocus
                        />
                      </div>
                      <div className="flex gap-2">
                        <Button onClick={handleCreateAccount} className="flex-1">
                          Create
                        </Button>
                        <Button variant="outline" onClick={() => setShowNewAccount(false)}>
                          Cancel
                        </Button>
                      </div>
                    </div>
                  </DialogContent>
                </Dialog>
              </div>

              {Object.keys(accounts).length === 0 ? (
                <div className="text-center py-12 text-muted-foreground">
                  <Wallet className="h-12 w-12 mx-auto mb-4 opacity-50" />
                  <p>No accounts yet</p>
                  <p className="text-sm mt-2">Create or import an account to get started</p>
                </div>
              ) : (
                <div className="space-y-3">
                  {Object.entries(accounts).map(([name, keyPair]) => (
                    <AccountCard
                      key={name}
                      name={name}
                      keyPair={keyPair}
                      selected={selectedAccount === name}
                      onSelect={() => setSelectedAccount(name)}
                      onDelete={() => {
                        if (confirm(`Delete account "${name}"?`)) {
                          deleteAccount(name);
                          toast.success("Account deleted");
                        }
                      }}
                    />
                  ))}
                </div>
              )}

              <Dialog open={showImport} onOpenChange={setShowImport}>
                <DialogTrigger asChild>
                  <Button variant="outline" className="w-full mt-4">
                    <Upload className="h-4 w-4 mr-2" />
                    Import Account
                  </Button>
                </DialogTrigger>
                <DialogContent>
                  <DialogHeader>
                    <DialogTitle>Import Account</DialogTitle>
                    <DialogDescription>
                      Import an existing account using a private key
                    </DialogDescription>
                  </DialogHeader>
                  <div className="space-y-4">
                    <div>
                      <Label htmlFor="import-name">Account Name</Label>
                      <Input
                        id="import-name"
                        value={importAccountName}
                        onChange={(e) => setImportAccountName(e.target.value)}
                        placeholder="e.g., Imported Wallet"
                      />
                    </div>
                    <div>
                      <Label htmlFor="import-key">Private Key (hex)</Label>
                      <Input
                        id="import-key"
                        value={importPrivateKey}
                        onChange={(e) => setImportPrivateKey(e.target.value)}
                        placeholder="64-character hex private key"
                        className="font-mono text-xs"
                      />
                    </div>
                    <div className="flex gap-2">
                      <Button onClick={handleImportAccount} className="flex-1">
                        Import
                      </Button>
                      <Button variant="outline" onClick={() => setShowImport(false)}>
                        Cancel
                      </Button>
                    </div>
                  </div>
                </DialogContent>
              </Dialog>
            </Card>

            {/* Right: Account Details */}
            <Card className="p-6">
              {selectedKeyPair ? (
                <AccountDetails
                  accountName={selectedAccount!}
                  keyPair={selectedKeyPair}
                  onSend={() => setShowSend(true)}
                />
              ) : (
                <div className="text-center py-12 text-muted-foreground">
                  <p>Select an account to view details</p>
                </div>
              )}
            </Card>
          </div>

          {selectedKeyPair && (
            <WalletMarketplaceSection
              address={selectedKeyPair.address}
              onCopy={(text, label) => {
                navigator.clipboard.writeText(text);
                toast.success(label);
              }}
            />
          )}

          {/* Send Transaction Modal */}
          {showSend && selectedKeyPair && (
            <SendTransactionModal
              accountName={selectedAccount!}
              keyPair={selectedKeyPair}
              onClose={() => setShowSend(false)}
            />
          )}
        </div>
      </div>
      <Footer />
    </div>
  );
}

interface AccountCardProps {
  name: string;
  keyPair: { address: string };
  selected: boolean;
  onSelect: () => void;
  onDelete: () => void;
}

function AccountCard({ name, keyPair, selected, onSelect, onDelete }: AccountCardProps) {
  const { data: balance } = useQuery({
    queryKey: ['balance', keyPair.address],
    queryFn: () => rpcClient.getBalance(keyPair.address),
    refetchInterval: 10000,
  });

  return (
    <Card
      className={`p-4 cursor-pointer transition-all ${
        selected ? 'ring-2 ring-primary' : 'hover:bg-muted/50'
      }`}
      onClick={onSelect}
    >
      <div className="flex items-start justify-between">
        <div className="flex-1">
          <div className="font-semibold mb-1">{name}</div>
          <div className="text-xs text-muted-foreground font-mono mb-2">
            {keyPair.address.slice(0, 16)}...{keyPair.address.slice(-8)}
          </div>
          <div className="text-lg font-bold text-primary">
            {balance !== undefined ? `${formatBeans(balance)} BEANS` : 'Loading...'}
          </div>
        </div>
        <Button
          variant="ghost"
          size="sm"
          onClick={(e) => {
            e.stopPropagation();
            onDelete();
          }}
          className="text-destructive hover:text-destructive"
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      </div>
    </Card>
  );
}

interface AccountDetailsProps {
  accountName: string;
  keyPair: { address: string; publicKey: string; privateKey: string };
  onSend: () => void;
}

function formatBeansLabel(raw: string | null | undefined): string | null {
  if (raw == null || raw === '') return null;
  try {
    const n = BigInt(raw);
    return `${formatBeans(n)} BEANS`;
  } catch {
    return `${raw} BEANS`;
  }
}

/** API may return legacy block_transaction rows without `kind` / `label`. */
function normalizeActivityKind(row: Partial<WalletActivityItem>): string {
  const k = row.kind;
  if (typeof k === 'string' && k.length > 0) return k;
  const t = row.tx_type;
  if (typeof t === 'string' && t.length > 0) return t.replace(/([a-z])([A-Z])/g, '$1_$2').toLowerCase();
  return 'chain_tx';
}

function normalizeActivityLabel(row: Partial<WalletActivityItem>): string {
  if (typeof row.label === 'string' && row.label.length > 0) return row.label;
  if (typeof row.tx_type === 'string' && row.tx_type.length > 0) return row.tx_type;
  return 'On-chain activity';
}

function activityRowKey(row: Partial<WalletActivityItem>, index: number): string {
  if (typeof row.id === 'string' && row.id.length > 0) return row.id;
  if (typeof row.tx_hash === 'string' && row.tx_hash.length > 0) return row.tx_hash;
  return `activity-${index}`;
}

function shortProblemId(hex: string): string {
  if (hex.length <= 22) return hex;
  return `${hex.slice(0, 12)}…${hex.slice(-8)}`;
}

function marketplaceStatusBadgeVariant(
  status: string,
): "default" | "secondary" | "destructive" | "outline" {
  const s = (status ?? "").toLowerCase();
  if (s === "open") return "default";
  if (s === "solved") return "secondary";
  if (s === "cancelled") return "destructive";
  return "outline";
}

function WalletMarketplaceSection({
  address,
  onCopy,
}: {
  address: string;
  onCopy: (text: string, label: string) => void;
}) {
  const posted = useQuery({
    queryKey: ["walletBountiesPosted", address],
    queryFn: () => rpcClient.getProblemsBySubmitter(address),
    refetchInterval: 20_000,
    retry: 1,
  });
  const solvedMine = useQuery({
    queryKey: ["walletBountiesSolved", address],
    queryFn: () => rpcClient.getProblemsBySolver(address),
    refetchInterval: 20_000,
    retry: 1,
  });

  return (
    <Card className="p-6 mt-6">
      <h2 className="text-2xl font-semibold mb-1">Your bounties & solutions</h2>
      <p className="text-sm text-muted-foreground mb-6 max-w-3xl">
        Problems you posted on the marketplace (any status), winning solutions attached to those
        listings when solved, and bounties where this wallet was the accepted solver.
      </p>
      <div className="grid gap-8 lg:grid-cols-2">
        <div>
          <h3 className="text-lg font-medium mb-3">Posted by you</h3>
          <BountyListColumn
            query={posted}
            emptyHint={
              <>
                No listings yet.{" "}
                <Link to="/bounty-submit" className="text-primary underline-offset-4 hover:underline">
                  Post a bounty
                </Link>{" "}
                or browse the{" "}
                <Link to="/marketplace" className="text-primary underline-offset-4 hover:underline">
                  live market
                </Link>
                .
              </>
            }
            mode="poster"
            onCopy={onCopy}
          />
        </div>
        <div>
          <h3 className="text-lg font-medium mb-3">You solved (repo)</h3>
          <BountyListColumn
            query={solvedMine}
            emptyHint={
              <>
                No accepted solutions yet. Open{" "}
                <Link to="/solver-lab" className="text-primary underline-offset-4 hover:underline">
                  Solver Lab
                </Link>{" "}
                from a live marketplace card.
              </>
            }
            mode="solver"
            onCopy={onCopy}
          />
        </div>
      </div>
    </Card>
  );
}

function BountyListColumn({
  query,
  emptyHint,
  mode,
  onCopy,
}: {
  query: {
    data: ProblemInfo[] | undefined;
    isLoading: boolean;
    isError: boolean;
    error: unknown;
  };
  emptyHint: ReactNode;
  mode: "poster" | "solver";
  onCopy: (text: string, label: string) => void;
}) {
  const { data, isLoading, isError, error } = query;
  const errMsg = error instanceof Error ? error.message : String(error ?? "");

  if (isLoading) {
    return <div className="text-sm text-muted-foreground">Loading…</div>;
  }
  if (isError) {
    return (
      <div className="text-sm text-muted-foreground space-y-1">
        <p>Could not load marketplace data from the node.</p>
        <p className="text-xs font-mono opacity-80">{errMsg || "Unknown error"}</p>
        <p className="text-xs">
          If the RPC method is missing, update the node to a build that includes{" "}
          <code className="text-[11px]">marketplace_getProblemsBySubmitter</code>.
        </p>
      </div>
    );
  }
  if (!data?.length) {
    return <div className="text-sm text-muted-foreground">{emptyHint}</div>;
  }

  return (
    <div className="space-y-3 max-h-[32rem] overflow-y-auto pr-1">
      {data.map((p) => (
        <div
          key={p.problem_id}
          className="rounded-lg border border-border/50 bg-muted/20 p-3 space-y-2 text-sm"
        >
          <div className="flex flex-wrap items-start justify-between gap-2">
            <div className="space-y-1 min-w-0">
              <div className="font-medium truncate">
                {p.problem_type ?? (p.is_private ? "Private bounty" : "Bounty")}
              </div>
              <div className="text-xs text-muted-foreground font-mono break-all">
                ID {shortProblemId(p.problem_id)}
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-6 px-1.5 ml-1"
                  onClick={() => onCopy(p.problem_id, "Problem ID copied")}
                >
                  <Copy className="h-3 w-3" />
                </Button>
              </div>
            </div>
            <Badge variant={marketplaceStatusBadgeVariant(p.status)}>{p.status}</Badge>
          </div>
          <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
            <span>
              <span className="text-foreground/80 font-medium">{p.bounty.toLocaleString()}</span>{" "}
              BEANS
            </span>
            {p.is_private && <span>{p.is_revealed ? "Revealed" : "Hidden until reveal"}</span>}
          </div>
          <div className="flex flex-wrap gap-2">
            <Button asChild variant="outline" size="sm">
              <Link to="/marketplace">Market</Link>
            </Button>
            {mode === "poster" &&
              isMarketplaceListingOpen(p.status) &&
              Boolean(p.problem) &&
              (!p.is_private || p.is_revealed) && (
                <Button asChild size="sm">
                  <Link
                    to={`/solver-lab?problemId=${encodeURIComponent(p.problem_id)}`}
                    state={{ selectedBounty: p }}
                  >
                    Solver Lab
                  </Link>
                </Button>
              )}
          </div>
          {mode === "poster" && p.status.toLowerCase() === "solved" && (p.solver || p.solution) && (
            <Collapsible>
              <CollapsibleTrigger asChild>
                <Button variant="ghost" size="sm" className="h-8 px-2 -ml-2 text-xs w-full justify-start">
                  <ChevronDown className="h-3.5 w-3.5 mr-1 shrink-0" />
                  Winning solution on your bounty
                </Button>
              </CollapsibleTrigger>
              <CollapsibleContent className="space-y-2 pt-1">
                {p.solver && (
                  <div className="text-xs text-muted-foreground break-all">
                    Solver: <span className="font-mono text-foreground/90">{p.solver}</span>
                  </div>
                )}
                {p.solution && (
                  <pre className="text-[11px] leading-relaxed font-mono bg-muted/50 rounded p-2 max-h-40 overflow-auto whitespace-pre-wrap break-all">
                    {JSON.stringify(p.solution, null, 2)}
                  </pre>
                )}
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  className="text-xs"
                  disabled={!p.solution}
                  onClick={() =>
                    p.solution &&
                    onCopy(JSON.stringify(p.solution), "Solution JSON copied")
                  }
                >
                  Copy solution JSON
                </Button>
              </CollapsibleContent>
            </Collapsible>
          )}
          {mode === "solver" && p.solution && (
            <Collapsible>
              <CollapsibleTrigger asChild>
                <Button variant="ghost" size="sm" className="h-8 px-2 -ml-2 text-xs w-full justify-start">
                  <ChevronDown className="h-3.5 w-3.5 mr-1 shrink-0" />
                  Your submitted solution
                </Button>
              </CollapsibleTrigger>
              <CollapsibleContent className="space-y-2 pt-1">
                <pre className="text-[11px] leading-relaxed font-mono bg-muted/50 rounded p-2 max-h-40 overflow-auto whitespace-pre-wrap break-all">
                  {JSON.stringify(p.solution, null, 2)}
                </pre>
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  className="text-xs"
                  onClick={() => onCopy(JSON.stringify(p.solution), "Solution JSON copied")}
                >
                  Copy solution JSON
                </Button>
              </CollapsibleContent>
            </Collapsible>
          )}
        </div>
      ))}
    </div>
  );
}

function activityKindStyles(kind: string): string {
  switch (kind) {
    case 'receive':
      return 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-400';
    case 'send':
    case 'self_transfer':
      return 'bg-sky-500/15 text-sky-800 dark:text-sky-300';
    case 'mining_reward':
      return 'bg-amber-500/15 text-amber-900 dark:text-amber-300';
    case 'marketplace':
      return 'bg-violet-500/15 text-violet-800 dark:text-violet-300';
    default:
      return 'bg-muted text-muted-foreground';
  }
}

function TransactionHistory({ address }: { address: string }) {
  const { data: items, isLoading, isError } = useQuery({
    queryKey: ['walletTxs', address],
    queryFn: () => getWalletTransactions(address),
    refetchInterval: 15_000,
    retry: 1,
  });

  if (isLoading) {
    return <div className="text-sm text-muted-foreground">Loading history...</div>;
  }
  if (isError) {
    return (
      <div className="text-sm text-muted-foreground">
        Could not load activity (check API / Supabase indexer).
      </div>
    );
  }
  if (!items?.length) {
    return <div className="text-sm text-muted-foreground">No on-chain activity yet</div>;
  }

  return (
    <div className="space-y-2">
      <Label className="text-muted-foreground">Recent activity</Label>
      <p className="text-xs text-muted-foreground">
        Sends, receives, block rewards when you mine, and marketplace / bounty actions indexed from the chain.
      </p>
      <div className="space-y-2 max-h-[28rem] overflow-y-auto pr-1">
        {items.map((row, index) => {
          if (row == null || typeof row !== 'object') return null;
          const kind = normalizeActivityKind(row);
          const label = normalizeActivityLabel(row);
          const amt = formatBeansLabel(row.amount);
          const fee = formatBeansLabel(row.fee);
          const cp =
            typeof row.counterparty === 'string'
              ? row.counterparty
              : row.counterparty != null
                ? String(row.counterparty)
                : null;
          const txRef =
            row.tx_hash && row.tx_hash.length > 14
              ? `${row.tx_hash.slice(0, 10)}…${row.tx_hash.slice(-6)}`
              : row.tx_hash || '—';
          const eventTypeStr = typeof row.event_type === 'string' ? row.event_type : '';
          const blockLabel =
            typeof row.block_height === 'number'
              ? row.block_height
              : row.block_height != null
                ? String(row.block_height)
                : '—';
          return (
            <div
              key={activityRowKey(row, index)}
              className="text-sm p-3 rounded-md bg-muted/30 border border-border/40 space-y-1.5"
            >
              <div className="flex flex-wrap items-start justify-between gap-2">
                <div className="flex flex-wrap items-center gap-2 min-w-0">
                  <span
                    className={`text-[10px] uppercase tracking-wide font-semibold px-2 py-0.5 rounded-md shrink-0 ${activityKindStyles(kind)}`}
                  >
                    {kind.replace(/_/g, ' ')}
                  </span>
                  <span className="font-medium leading-snug">{label}</span>
                </div>
                <span className="text-muted-foreground text-xs shrink-0 tabular-nums">
                  Block {blockLabel}
                </span>
              </div>
              {(amt || fee || cp) && (
                <div className="text-xs text-muted-foreground space-y-0.5">
                  {amt && (
                    <div>
                      <span className="text-foreground/80 font-medium">{amt}</span>
                      {fee && kind === 'send' && (
                        <span className="ml-2">(fee {fee})</span>
                      )}
                    </div>
                  )}
                  {cp && (kind === 'send' || kind === 'receive') && (
                    <div className="font-mono">
                      {kind === 'send' ? 'To' : 'From'}: {cp}
                    </div>
                  )}
                  {kind === 'marketplace' && eventTypeStr && (
                    <div className="capitalize">
                      Event: {eventTypeStr.replace(/_/g, ' ')}
                    </div>
                  )}
                  {kind === 'marketplace' && row.problem_id != null && String(row.problem_id) !== '' && (
                    <div className="font-mono break-all">Problem: {String(row.problem_id)}</div>
                  )}
                </div>
              )}
              <div className="font-mono text-[11px] text-muted-foreground/90 break-all">
                {row.tx_hash ? `Tx ${txRef}` : 'No transaction hash (block reward)'}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function AccountDetails({ accountName, keyPair, onSend }: AccountDetailsProps) {
  const [showPrivateKey, setShowPrivateKey] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [copied, setCopied] = useState<string | null>(null);
  const queryClient = useQueryClient();

  const { data: accountInfo } = useQuery({
    queryKey: ['accountInfo', keyPair.address],
    queryFn: () => rpcClient.getAccountInfo(keyPair.address),
    refetchInterval: 10000,
  });

  const faucetMutation = useMutation({
    mutationFn: () => rpcClient.faucetRequestTokens(keyPair.address),
    onSuccess: (response) => {
      if (response.success) {
        toast.success(`${response.message}`);
        queryClient.invalidateQueries({ queryKey: ['accountInfo', keyPair.address] });
        queryClient.invalidateQueries({ queryKey: ['balance', keyPair.address] });
        queryClient.invalidateQueries({ queryKey: ['walletTxs', keyPair.address] });
      } else {
        toast.error(response.message);
      }
    },
    onError: (error: any) => {
      toast.error(`Faucet request failed: ${error.message || 'Unknown error'}`);
    },
  });

  const copyToClipboard = (text: string, label: string) => {
    navigator.clipboard.writeText(text);
    setCopied(label);
    setTimeout(() => setCopied(null), 2000);
    toast.success("Copied to clipboard");
  };

  return (
    <div>
      <h3 className="text-xl font-semibold mb-6">Account: {accountName}</h3>

      <div className="space-y-4">
        <div>
          <Label className="text-muted-foreground">Balance</Label>
          <div className="text-3xl font-bold text-primary mt-1">
            {accountInfo ? `${formatBeans(accountInfo.balance)} BEANS` : 'Loading...'}
          </div>
        </div>

        <div>
          <Label className="text-muted-foreground">Nonce</Label>
          <div className="font-mono text-sm mt-1">{accountInfo?.nonce ?? 'Loading...'}</div>
        </div>

        <div className="space-y-6 pt-4 border-t border-border/60">
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label className="text-base font-semibold">Receive BEANS</Label>
            </div>
            <p className="text-sm text-muted-foreground">
              Share your address to receive BEANS from others
            </p>
            <div className="flex items-start gap-2 p-3 bg-muted/50 rounded-lg">
              <code className="flex-1 text-xs font-mono break-all leading-relaxed min-w-0">
                {keyPair.address}
              </code>
              <Button
                variant="outline"
                size="sm"
                className="shrink-0"
                onClick={() => copyToClipboard(keyPair.address, 'receive-address')}
              >
                {copied === 'receive-address' ? (
                  <Check className="h-4 w-4" />
                ) : (
                  <Copy className="h-4 w-4" />
                )}
              </Button>
            </div>
          </div>

          <div className="flex gap-2">
            <Button onClick={onSend} className="flex-1">
              <Send className="h-4 w-4 mr-2" />
              Send BEANS
            </Button>
            <Button
              variant="outline"
              onClick={() => faucetMutation.mutate()}
              disabled={faucetMutation.isPending}
            >
              💧 {faucetMutation.isPending ? 'Requesting...' : 'Faucet'}
            </Button>
          </div>
        </div>

        <div className="pt-2">
          <TransactionHistory address={keyPair.address} />
        </div>

        <Collapsible open={advancedOpen} onOpenChange={setAdvancedOpen}>
          <CollapsibleTrigger asChild>
            <Button variant="ghost" className="w-full justify-between px-0 text-muted-foreground hover:text-foreground">
              <span className="text-sm font-medium">Advanced — keys and raw address</span>
              <ChevronDown
                className={`h-4 w-4 shrink-0 transition-transform ${advancedOpen ? 'rotate-180' : ''}`}
              />
            </Button>
          </CollapsibleTrigger>
          <CollapsibleContent className="space-y-4 pt-2">
            <div>
              <Label className="text-muted-foreground">Address</Label>
              <div className="flex items-center gap-2 mt-1">
                <code className="flex-1 text-xs font-mono break-all">{keyPair.address}</code>
                <Button variant="ghost" size="sm" onClick={() => copyToClipboard(keyPair.address, 'address')}>
                  {copied === 'address' ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
                </Button>
              </div>
            </div>

            <div>
              <Label className="text-muted-foreground">Public Key</Label>
              <div className="flex items-center gap-2 mt-1">
                <code className="flex-1 text-xs font-mono break-all">{keyPair.publicKey}</code>
                <Button variant="ghost" size="sm" onClick={() => copyToClipboard(keyPair.publicKey, 'public')}>
                  {copied === 'public' ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
                </Button>
              </div>
            </div>

            <div>
              <div className="flex items-center justify-between mb-2">
                <Label className="text-muted-foreground">Private Key</Label>
                <Button variant="ghost" size="sm" onClick={() => setShowPrivateKey(!showPrivateKey)}>
                  {showPrivateKey ? (
                    <>
                      <EyeOff className="h-4 w-4 mr-2" />
                      Hide
                    </>
                  ) : (
                    <>
                      <Eye className="h-4 w-4 mr-2" />
                      Show
                    </>
                  )}
                </Button>
              </div>
              {showPrivateKey ? (
                <div className="flex items-center gap-2">
                  <code className="flex-1 text-xs font-mono break-all bg-destructive/10 p-2 rounded">
                    {keyPair.privateKey}
                  </code>
                  <Button variant="ghost" size="sm" onClick={() => copyToClipboard(keyPair.privateKey, 'private')}>
                    {copied === 'private' ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
                  </Button>
                </div>
              ) : (
                <code className="text-xs font-mono">••••••••••••••••••••••••••••••••</code>
              )}
              {showPrivateKey && (
                <p className="text-xs text-destructive mt-2">⚠️ Never share your private key!</p>
              )}
            </div>
          </CollapsibleContent>
        </Collapsible>
      </div>
    </div>
  );
}

interface SendTransactionModalProps {
  accountName: string;
  keyPair: { address: string; privateKey: string; publicKey: string };
  onClose: () => void;
}

function SendTransactionModal({ accountName, keyPair, onClose }: SendTransactionModalProps) {
  const [recipient, setRecipient] = useState("");
  const [amount, setAmount] = useState("");
  /** When on, sign with a tip only (skips zero-fee attempt). When off, try zero fee first, then tip if the pool rejects. */
  const [priorityTip, setPriorityTip] = useState(false);
  const queryClient = useQueryClient();

  const {
    data: accountInfo,
    isPending: accountInfoPending,
    isError: accountInfoError,
  } = useQuery({
    queryKey: ['accountInfo', keyPair.address],
    queryFn: () => rpcClient.getAccountInfo(keyPair.address),
  });

  const sendMutation = useMutation({
    mutationFn: async () => {
      if (!accountInfo) throw new Error("Account info not loaded");

      const amountDisplay = BigInt(amount.trim());
      if (amountDisplay <= 0n) throw new Error("Invalid amount");
      const amountAtoms = displayBeansToAtoms(amountDisplay);
      const { address: from, privateKey, publicKey } = keyPair;
      const { nonce } = accountInfo;

      const sign = (fee: bigint) =>
        createSignedTransferTransaction(from, recipient, amountAtoms, fee, nonce, privateKey, publicKey);

      if (priorityTip) {
        return rpcClient.submitTransaction(sign(TRANSFER_PRIORITY_TIP_ATOMS));
      }

      try {
        return await rpcClient.submitTransaction(sign(0n));
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        if (!isPoolFeeTooLowMessage(msg)) throw e;
        return rpcClient.submitTransaction(sign(TRANSFER_PRIORITY_TIP_ATOMS));
      }
    },
    onSuccess: (txHash) => {
      toast.success("Transfer queued in mempool", {
        description: `Hash ${txHash.slice(0, 16)}… Balances change after a miner includes this transaction in a block (not when you see this message).`,
      });
      queryClient.invalidateQueries({ queryKey: ['accountInfo', keyPair.address] });
      queryClient.invalidateQueries({ queryKey: ['balance', keyPair.address] });
      queryClient.invalidateQueries({ queryKey: ['walletTxs', keyPair.address] });
      onClose();
    },
    onError: (error: any) => {
      toast.error(error.message || "Transaction failed");
    },
  });

  const accountInfoLoading = accountInfoPending && !accountInfo;
  const sendDisabled =
    sendMutation.isPending || !accountInfo || accountInfoLoading || accountInfoError;

  const handleSubmit = () => {
    if (!accountInfo) {
      if (accountInfoError) {
        toast.error("Could not load account info. Check your connection and try again.");
      } else {
        toast.error("Account info is still loading — please wait a moment.");
      }
      return;
    }
    if (!recipient.match(/^[0-9a-f]{64}$/i)) {
      toast.error("Invalid recipient address (must be 64-character hex)");
      return;
    }
    let amountDisplay: bigint;
    try {
      amountDisplay = BigInt(amount.trim());
    } catch {
      toast.error("Invalid amount");
      return;
    }
    if (!amount || amountDisplay <= 0n) {
      toast.error("Invalid amount");
      return;
    }
    const amountAtoms = displayBeansToAtoms(amountDisplay);
    if (accountInfo.balance < amountAtoms) {
      toast.error("Insufficient balance for this amount");
      return;
    }
    if (priorityTip && accountInfo.balance < amountAtoms + TRANSFER_PRIORITY_TIP_ATOMS) {
      toast.error(
        `Need at least ${formatBeans(amountAtoms + TRANSFER_PRIORITY_TIP_ATOMS)} BEANS (amount + ${formatBeans(TRANSFER_PRIORITY_TIP_ATOMS)} tip)`
      );
      return;
    }
    sendMutation.mutate();
  };

  const sendLabel = sendMutation.isPending
    ? "Sending..."
    : accountInfoLoading
      ? "Loading..."
      : "Send";

  return (
    <Dialog open={true} onOpenChange={onClose}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Send Transaction</DialogTitle>
          <DialogDescription>
            Send BEANS from {accountName}. By default the wallet tries a <strong>zero-fee</strong> transfer (supported
            on updated nodes with fair block templates). Turn on the tip below to attach{" "}
            {formatBeans(TRANSFER_PRIORITY_TIP_ATOMS)} BEANS for stronger fee-priority or for older nodes that
            reject zero-fee submits.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-4">
          <div>
            <Label htmlFor="recipient">Recipient Address</Label>
            <Input
              id="recipient"
              value={recipient}
              onChange={(e) => setRecipient(e.target.value)}
              placeholder="64-character hex address"
              className="font-mono text-xs"
            />
          </div>
          <div>
            <Label htmlFor="amount">Amount</Label>
            <Input
              id="amount"
              type="number"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              min="1"
            />
          </div>
          <div className="flex items-center justify-between gap-3 rounded-md border p-3">
            <div className="space-y-0.5">
              <Label htmlFor="priority-tip" className="text-sm font-medium">
                Faster confirmation ({formatBeans(TRANSFER_PRIORITY_TIP_ATOMS)} BEANS tip)
              </Label>
              <p className="text-xs text-muted-foreground">
                Skip zero-fee and pay the miner tip up front. Optional on networks that already accept free transfers.
              </p>
            </div>
            <Switch id="priority-tip" checked={priorityTip} onCheckedChange={setPriorityTip} />
          </div>
          {accountInfoError && (
            <p className="text-sm text-destructive">Failed to load account info. Close and try again.</p>
          )}
          {accountInfo && (
            <div className="text-sm text-muted-foreground">
              Balance: {formatBeans(accountInfo.balance)} BEANS | Nonce: {accountInfo.nonce}
            </div>
          )}
          <div className="flex gap-2">
            <Button onClick={handleSubmit} disabled={sendDisabled} className="flex-1">
              {sendLabel}
            </Button>
            <Button variant="outline" onClick={onClose}>
              Cancel
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

