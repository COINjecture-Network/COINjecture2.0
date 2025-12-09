import { Navigation } from "@/components/Navigation";
import { Footer } from "@/components/Footer";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { useWallet } from "@/contexts/WalletContext";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { rpcClient } from "@/lib/rpc-client";
import { Wallet, Plus, Upload, Send, Copy, Trash2, Eye, EyeOff, Check } from "lucide-react";
import { useState } from "react";
import { createSignedTransferTransaction } from "@/lib/wallet-crypto";
import { toast } from "sonner";

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
      <div className="pt-24 pb-20 container mx-auto px-6">
        <div className="max-w-6xl mx-auto">
          <h1 className="text-4xl font-bold mb-8">Wallet</h1>

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
            {balance !== undefined ? `${balance.toLocaleString()} BEANS` : 'Loading...'}
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

function AccountDetails({ accountName, keyPair, onSend }: AccountDetailsProps) {
  const [showPrivateKey, setShowPrivateKey] = useState(false);
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
            {accountInfo ? `${accountInfo.balance.toLocaleString()} BEANS` : 'Loading...'}
          </div>
        </div>

        <div>
          <Label className="text-muted-foreground">Nonce</Label>
          <div className="font-mono text-sm mt-1">{accountInfo?.nonce ?? 'Loading...'}</div>
        </div>

        <div>
          <Label className="text-muted-foreground">Address</Label>
          <div className="flex items-center gap-2 mt-1">
            <code className="flex-1 text-xs font-mono break-all">{keyPair.address}</code>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => copyToClipboard(keyPair.address, 'address')}
            >
              {copied === 'address' ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
            </Button>
          </div>
        </div>

        <div>
          <Label className="text-muted-foreground">Public Key</Label>
          <div className="flex items-center gap-2 mt-1">
            <code className="flex-1 text-xs font-mono break-all">{keyPair.publicKey}</code>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => copyToClipboard(keyPair.publicKey, 'public')}
            >
              {copied === 'public' ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
            </Button>
          </div>
        </div>

        <div>
          <div className="flex items-center justify-between mb-2">
            <Label className="text-muted-foreground">Private Key</Label>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setShowPrivateKey(!showPrivateKey)}
            >
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
              <Button
                variant="ghost"
                size="sm"
                onClick={() => copyToClipboard(keyPair.privateKey, 'private')}
              >
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

        <div className="flex gap-2 pt-4">
          <Button onClick={onSend} className="flex-1">
            <Send className="h-4 w-4 mr-2" />
            Send Transaction
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
  const [fee, setFee] = useState("1500");
  const queryClient = useQueryClient();

  const { data: accountInfo } = useQuery({
    queryKey: ['accountInfo', keyPair.address],
    queryFn: () => rpcClient.getAccountInfo(keyPair.address),
  });

  const sendMutation = useMutation({
    mutationFn: async () => {
      if (!accountInfo) throw new Error("Account info not loaded");
      
      const signedTx = createSignedTransferTransaction(
        keyPair.address,
        recipient,
        parseInt(amount),
        parseInt(fee),
        accountInfo.nonce,
        keyPair.privateKey,
        keyPair.publicKey
      );

      return rpcClient.submitTransaction(signedTx);
    },
    onSuccess: (txHash) => {
      toast.success(`Transaction submitted! Hash: ${txHash.slice(0, 16)}...`);
      queryClient.invalidateQueries({ queryKey: ['accountInfo', keyPair.address] });
      queryClient.invalidateQueries({ queryKey: ['balance', keyPair.address] });
      onClose();
    },
    onError: (error: any) => {
      toast.error(error.message || "Transaction failed");
    },
  });

  const handleSubmit = () => {
    if (!recipient.match(/^[0-9a-f]{64}$/i)) {
      toast.error("Invalid recipient address (must be 64-character hex)");
      return;
    }
    if (!amount || parseInt(amount) <= 0) {
      toast.error("Invalid amount");
      return;
    }
    sendMutation.mutate();
  };

  return (
    <Dialog open={true} onOpenChange={onClose}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Send Transaction</DialogTitle>
          <DialogDescription>
            Send BEANS tokens from {accountName}
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
          <div className="grid grid-cols-2 gap-4">
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
            <div>
              <Label htmlFor="fee">Fee</Label>
              <Input
                id="fee"
                type="number"
                value={fee}
                onChange={(e) => setFee(e.target.value)}
                min="0"
              />
            </div>
          </div>
          {accountInfo && (
            <div className="text-sm text-muted-foreground">
              Balance: {accountInfo.balance.toLocaleString()} BEANS | Nonce: {accountInfo.nonce}
            </div>
          )}
          <div className="flex gap-2">
            <Button onClick={handleSubmit} disabled={sendMutation.isPending} className="flex-1">
              {sendMutation.isPending ? "Sending..." : "Send"}
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

