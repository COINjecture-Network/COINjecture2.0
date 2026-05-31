import { Navigation } from "@/components/Navigation";
import { Footer } from "@/components/Footer";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { useToast } from "@/hooks/use-toast";
import { useState, useEffect, useMemo } from "react";
import { Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { CheckCircle2, Circle, XCircle } from "lucide-react";
import type { ProblemType } from "@/lib/rpc-client";
import { rpcClient } from "@/lib/rpc-client";
import {
  BOUNTY_PROBLEM_KINDS,
  applyTemplateToForm,
  bountyKindMeta,
  resolveBountyProblem,
  splitLegacyDescription,
  templatesForKind,
  type BountyProblemKind,
} from "@/lib/bounty";
import {
  displayBeansToAtoms,
  formatBeans,
  MIN_BOUNTY_SUBMISSION_FEE_ATOMS,
  parseBalance,
} from "@/lib/chain-metrics";
import { isMarketplaceListingOpen } from "@/lib/marketplace-status";
import { useWallet } from "@/contexts/WalletContext";

/** Must match `STORAGE_KEY` in `NpPlayground.tsx` (Solver Lab → Bounty draft). */
const SOLVER_LAB_BOUNTY_KEY = "solverLabBountyPayload";
/** On-chain `MIN_FEE_BOUNTY_SUBMISSION` when posting via `transaction_submit` — re-exported from chain-metrics. */
const MARKETPLACE_SUBMIT_FEE_ATOMS = MIN_BOUNTY_SUBMISSION_FEE_ATOMS;

type ConfirmedSubmission = {
  problemId: string;
  title: string;
  bounty: number;
  submitter: string;
  mode: "public" | "private";
  commitment?: string;
  salt?: string;
  problemJson: string;
};

type RevealFormData = {
  problemId: string;
  salt: string;
  problemJson: string;
};

type StoredRevealKit = {
  problemId: string;
  submitter: string;
  title: string;
  salt: string;
  problemJson: string;
  commitment?: string;
  createdAt: number;
};

const defaultFormData = {
  title: "",
  problemType: "SubsetSum" as BountyProblemKind,
  briefing: "",
  instanceJson: "",
  bounty: "",
  minWorkScore: "100",
  submissionMode: "public",
  expirationDays: "30",
  complexity: "medium",
  priority: "standard",
  verificationMethod: "automated",
  aggregationMethod: "best_solution",
  notes: "",
};

const PRIVATE_REVEAL_KITS_KEY = "coinjecturePrivateRevealKits";

function ChecklistRow({
  ok,
  label,
  detail,
}: {
  ok: boolean | null;
  label: string;
  detail?: string;
}) {
  const Icon = ok === null ? Circle : ok ? CheckCircle2 : XCircle;
  const tone =
    ok === null
      ? "text-muted-foreground"
      : ok
        ? "text-emerald-600 dark:text-emerald-400"
        : "text-destructive";
  return (
    <div className="flex gap-3 text-sm">
      <Icon className={`mt-0.5 h-4 w-4 shrink-0 ${tone}`} />
      <div>
        <div className={ok ? "font-medium text-foreground" : "text-muted-foreground"}>{label}</div>
        {detail ? <div className="text-xs text-muted-foreground mt-0.5">{detail}</div> : null}
      </div>
    </div>
  );
}

function generateSaltHex(): string {
  const salt = new Uint8Array(32);
  crypto.getRandomValues(salt);
  return `0x${Array.from(salt, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

async function confirmProblemCreated(problemId: string) {
  for (let attempt = 0; attempt < 4; attempt += 1) {
    const problem = await rpcClient.getProblem(problemId);
    if (problem) {
      return problem;
    }
    await new Promise((resolve) => window.setTimeout(resolve, 500));
  }

  throw new Error("The bounty submission reached the node, but confirmation has not appeared yet. Please refresh the marketplace in a moment.");
}

function loadStoredRevealKits(): Record<string, StoredRevealKit> {
  try {
    const raw = localStorage.getItem(PRIVATE_REVEAL_KITS_KEY);
    if (!raw) {
      return {};
    }

    const parsed = JSON.parse(raw) as Record<string, StoredRevealKit>;
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch {
    return {};
  }
}

function persistStoredRevealKits(kits: Record<string, StoredRevealKit>) {
  localStorage.setItem(PRIVATE_REVEAL_KITS_KEY, JSON.stringify(kits));
}

function revealKitKey(submitter: string, problemId: string): string {
  return `${submitter.toLowerCase()}:${problemId.toLowerCase()}`;
}

const BountySubmit = () => {
  const { toast } = useToast();
  const { accounts, selectedAccount } = useWallet();
  const selectedKeyPair = selectedAccount ? accounts[selectedAccount] : null;
  const [formData, setFormData] = useState(defaultFormData);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [confirmedSubmission, setConfirmedSubmission] = useState<ConfirmedSubmission | null>(null);
  const [revealForm, setRevealForm] = useState<RevealFormData>({
    problemId: "",
    salt: "",
    problemJson: "",
  });
  const [isRevealing, setIsRevealing] = useState(false);
  const [revealError, setRevealError] = useState<string | null>(null);
  const [revealedProblemId, setRevealedProblemId] = useState<string | null>(null);
  const [copiedField, setCopiedField] = useState<string | null>(null);
  const [storedRevealKits, setStoredRevealKits] = useState<Record<string, StoredRevealKit>>({});

  const totalEscrowAtoms = formData.bounty.trim()
    ? displayBeansToAtoms(BigInt(formData.bounty.trim())) + MARKETPLACE_SUBMIT_FEE_ATOMS
    : MARKETPLACE_SUBMIT_FEE_ATOMS;
  const { data: marketplaceStats, refetch: refetchMarketplaceStats } = useQuery({
    queryKey: ["marketplace-stats"],
    queryFn: () => rpcClient.getMarketplaceStats(),
    refetchInterval: 30000,
  });
  const { data: walletBalance, refetch: refetchWalletBalance } = useQuery({
    queryKey: ["wallet-balance", selectedKeyPair?.address],
    queryFn: () => rpcClient.getBalance(selectedKeyPair!.address),
    enabled: Boolean(selectedKeyPair?.address),
    refetchInterval: 30000,
  });
  const { data: openProblems, refetch: refetchOpenProblems } = useQuery({
    queryKey: ["marketplace-open-problems"],
    queryFn: () => rpcClient.getOpenProblems(),
    refetchInterval: 30000,
  });
  const problemTypeOptions = BOUNTY_PROBLEM_KINDS;
  const kindMeta = bountyKindMeta(formData.problemType);
  const kindTemplates = templatesForKind(formData.problemType);
  const instancePreview = useMemo(
    () => resolveBountyProblem(formData.instanceJson, formData.briefing, formData.problemType),
    [formData.instanceJson, formData.briefing, formData.problemType],
  );
  const hasWallet = Boolean(selectedKeyPair?.address);
  const hasTitle = formData.title.trim().length > 0;
  const bountyNum = Number.parseInt(formData.bounty, 10);
  const hasValidBounty = Number.isFinite(bountyNum) && bountyNum >= 1;
  const hasSufficientBalance =
    walletBalance !== undefined && hasValidBounty && walletBalance >= totalEscrowAtoms;
  const canSubmit =
    hasWallet && hasTitle && instancePreview.ok && hasValidBounty && hasSufficientBalance && !isSubmitting;
  const rewardPresets = ["1", "2", "5", "10", "25"];
  const durationPresets = ["7", "14", "30", "90"];
  const complexityOptions = [
    { value: "easy", label: "Easy" },
    { value: "medium", label: "Medium" },
    { value: "hard", label: "Hard" },
    { value: "expert", label: "Expert" },
  ];
  const priorityOptions = [
    { value: "low", label: "Low" },
    { value: "standard", label: "Standard" },
    { value: "high", label: "High" },
    { value: "urgent", label: "Urgent" },
  ];
  const selectedProblemType = problemTypeOptions.find((option) => option.value === formData.problemType);
  const myPrivateBounties = (openProblems ?? []).filter((problem) => {
    if (!selectedKeyPair?.address) {
      return false;
    }

    return (
      problem.submitter.toLowerCase() === selectedKeyPair.address.toLowerCase() &&
      problem.is_private &&
      !problem.is_revealed &&
      isMarketplaceListingOpen(problem.status)
    );
  });

  useEffect(() => {
    setStoredRevealKits(loadStoredRevealKits());
  }, []);

  useEffect(() => {
    try {
      const raw = sessionStorage.getItem(SOLVER_LAB_BOUNTY_KEY);
      if (!raw) return;
      const data = JSON.parse(raw) as {
        problemType?: string;
        title?: string;
        description?: string;
        briefing?: string;
        instanceJson?: string;
        draftKind?: "problem" | "solver";
      };
      sessionStorage.removeItem(SOLVER_LAB_BOUNTY_KEY);
      if (data.title && (data.description || data.briefing || data.instanceJson)) {
        const legacy = data.description ?? "";
        const split = splitLegacyDescription(legacy);
        setFormData((prev) => ({
          ...prev,
          title: data.title!,
          problemType: (data.problemType as BountyProblemKind) ?? prev.problemType,
          briefing: data.briefing ?? split.briefing ?? legacy,
          instanceJson: data.instanceJson ?? split.instanceJson ?? prev.instanceJson,
        }));
        const isProblemOnly = data.draftKind === "problem";
        toast({
          title: isProblemOnly ? "Problem draft from Solver Lab" : "Draft loaded from Solver Lab",
          description: isProblemOnly
            ? "Instance JSON only — set bounty and escrow, then submit on-chain."
            : "Review the instance JSON, set bounty and escrow, then submit.",
        });
      }
    } catch {
      sessionStorage.removeItem(SOLVER_LAB_BOUNTY_KEY);
    }
  }, [toast]);

  useEffect(() => {
    if (!confirmedSubmission || confirmedSubmission.mode !== "private") {
      return;
    }

    setRevealForm({
      problemId: confirmedSubmission.problemId,
      salt: confirmedSubmission.salt ?? "",
      problemJson: confirmedSubmission.problemJson,
    });
    setRevealError(null);
    setRevealedProblemId(null);
  }, [confirmedSubmission]);

  const storeRevealKit = (kit: StoredRevealKit) => {
    setStoredRevealKits((current) => {
      const next = {
        ...current,
        [revealKitKey(kit.submitter, kit.problemId)]: kit,
      };
      persistStoredRevealKits(next);
      return next;
    });
  };

  const removeRevealKit = (submitter: string, problemId: string) => {
    setStoredRevealKits((current) => {
      const next = { ...current };
      delete next[revealKitKey(submitter, problemId)];
      persistStoredRevealKits(next);
      return next;
    });
  };

  const loadRevealKitIntoForm = (problemId: string) => {
    const savedKit = selectedKeyPair?.address
      ? storedRevealKits[revealKitKey(selectedKeyPair.address, problemId)]
      : undefined;

    setRevealForm({
      problemId,
      salt: savedKit?.salt ?? "",
      problemJson: savedKit?.problemJson ?? "",
    });

    if (savedKit) {
      toast({
        title: "Reveal kit loaded",
        description: `Loaded saved salt and problem JSON for ${problemId.slice(0, 12)}...`,
      });
    } else {
      toast({
        title: "Problem selected",
        description: "No saved reveal kit was found locally for this bounty yet.",
      });
    }
  };

  const loadTemplate = (templateId: string) => {
    const template = kindTemplates.find((t) => t.id === templateId) ?? kindTemplates[0];
    if (!template) return;
    setFormData((prev) => ({
      ...prev,
      ...applyTemplateToForm(template),
      submissionMode: prev.submissionMode,
      priority: prev.priority,
      verificationMethod: prev.verificationMethod,
      aggregationMethod: prev.aggregationMethod,
      notes: prev.notes,
    }));
    toast({
      title: "Example loaded",
      description: `${template.label} — edit the instance JSON or briefing, then set escrow.`,
    });
  };

  const copyToClipboard = async (value: string, field: string) => {
    try {
      await navigator.clipboard.writeText(value);
      setCopiedField(field);
      window.setTimeout(() => setCopiedField((current) => (current === field ? null : current)), 2000);
      toast({
        title: "Copied",
        description: `${field} copied to your clipboard.`,
      });
    } catch {
      toast({
        title: "Copy failed",
        description: `Unable to copy ${field.toLowerCase()} automatically.`,
        variant: "destructive",
      });
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    setSubmitError(null);
    setConfirmedSubmission(null);

    if (!selectedKeyPair?.address) {
      const message = "Connect a wallet before submitting a bounty so the escrow can be assigned to your account.";
      setSubmitError(message);
      toast({ title: "Wallet required", description: message, variant: "destructive" });
      return;
    }

    const bounty = Number.parseInt(formData.bounty, 10);
    const minWorkScore = Number.parseFloat(formData.minWorkScore);
    const expirationDays = Number.parseInt(formData.expirationDays, 10);

    if (!Number.isFinite(bounty) || bounty < 1) {
      setSubmitError("Bounty must be at least 1 BEANS.");
      return;
    }

    if (!Number.isFinite(minWorkScore) || minWorkScore <= 0) {
      setSubmitError("Minimum work score must be greater than zero.");
      return;
    }

    if (!Number.isFinite(expirationDays) || expirationDays < 1 || expirationDays > 365) {
      setSubmitError("Expiration must be between 1 and 365 days.");
      return;
    }

    const bountyAtoms = displayBeansToAtoms(BigInt(bounty));
    const requiredAtoms = bountyAtoms + MARKETPLACE_SUBMIT_FEE_ATOMS;
    if (walletBalance !== undefined && walletBalance < requiredAtoms) {
      const message = `Insufficient wallet balance. Available: ${formatBeans(walletBalance)} BEANS, required: ${formatBeans(requiredAtoms)} BEANS (bounty + network fee).`;
      setSubmitError(message);
      toast({ title: "Insufficient balance", description: message, variant: "destructive" });
      return;
    }

    const parsed = resolveBountyProblem(
      formData.instanceJson,
      formData.briefing,
      formData.problemType,
    );
    if (!parsed.ok) {
      setSubmitError(parsed.error);
      toast({ title: "Invalid instance JSON", description: parsed.error, variant: "destructive" });
      return;
    }
    const parsedProblem = parsed.problem;

    setIsSubmitting(true);

    try {
      let problemId: string;
      let commitment: string | undefined;
      let salt: string | undefined;

      const bountyParam = displayBeansToAtoms(BigInt(bounty)).toString();
      if (formData.submissionMode === "private") {
        salt = generateSaltHex();
        const privateResult = await rpcClient.submitPrivateProblemWithWallet({
          problem: parsedProblem,
          salt,
          bounty: bountyParam,
          min_work_score: minWorkScore,
          expiration_days: expirationDays,
          submitter: selectedKeyPair.address,
        });
        problemId = privateResult.problem_id;
        commitment = privateResult.commitment;
      } else {
        problemId = await rpcClient.submitPublicProblem({
          problem: parsedProblem,
          bounty: bountyParam,
          min_work_score: minWorkScore,
          expiration_days: expirationDays,
          submitter: selectedKeyPair.address,
        });
      }

      await confirmProblemCreated(problemId);
      await Promise.all([refetchMarketplaceStats(), refetchWalletBalance(), refetchOpenProblems()]);

      setConfirmedSubmission({
        problemId,
        title: formData.title,
        bounty,
        submitter: selectedKeyPair.address,
        mode: formData.submissionMode as "public" | "private",
        commitment,
        salt,
        problemJson: JSON.stringify(parsedProblem),
      });
      if (formData.submissionMode === "private" && salt) {
        storeRevealKit({
          problemId,
          submitter: selectedKeyPair.address,
          title: formData.title,
          salt,
          problemJson: JSON.stringify(parsedProblem),
          commitment,
          createdAt: Date.now(),
        });
      }
      setFormData(defaultFormData);

      toast({
        title: "Bounty confirmed on-chain",
        description:
          formData.submissionMode === "private"
            ? `Private commitment ${problemId.slice(0, 12)}... is live. Save the reveal salt before you leave this page.`
            : `Escrow locked and problem ${problemId.slice(0, 12)}... is now live in the marketplace.`,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to submit bounty.";
      setSubmitError(message);
      toast({ title: "Submission failed", description: message, variant: "destructive" });
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleReveal = async (e: React.FormEvent) => {
    e.preventDefault();
    setRevealError(null);
    setRevealedProblemId(null);

    if (!revealForm.problemId.trim()) {
      setRevealError("Enter the private bounty problem ID you want to reveal.");
      return;
    }
    if (!revealForm.salt.trim()) {
      setRevealError("Enter the 32-byte reveal salt for this private bounty.");
      return;
    }
    if (!revealForm.problemJson.trim()) {
      setRevealError("Paste the exact problem JSON that matches the original private commitment.");
      return;
    }

    let normalizedProblemJson: string;
    try {
      normalizedProblemJson = JSON.stringify(JSON.parse(revealForm.problemJson));
    } catch {
      setRevealError("Problem JSON must be valid JSON before it can be revealed.");
      return;
    }

    setIsRevealing(true);

    try {
      await rpcClient.revealProblem({
        problem_id: revealForm.problemId.trim(),
        problem: normalizedProblemJson,
        salt: revealForm.salt.trim(),
      });

      const problem = await confirmProblemCreated(revealForm.problemId.trim());
      if (!problem.is_revealed) {
        throw new Error("The node accepted the reveal request, but the problem is not marked as revealed yet.");
      }

      setRevealedProblemId(revealForm.problemId.trim());
      if (selectedKeyPair?.address) {
        removeRevealKit(selectedKeyPair.address, revealForm.problemId.trim());
      }
      await Promise.all([refetchMarketplaceStats(), refetchOpenProblems()]);

      toast({
        title: "Private bounty revealed",
        description: `Problem ${revealForm.problemId.trim().slice(0, 12)}... is now visible to solvers.`,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to reveal private bounty.";
      setRevealError(message);
      toast({
        title: "Reveal failed",
        description: message,
        variant: "destructive",
      });
    } finally {
      setIsRevealing(false);
    }
  };

  return (
    <div className="min-h-screen">
      <Navigation />
      <main className="pt-32 pb-20">
        <div className="container mx-auto px-6">
          <div className="max-w-4xl mx-auto">
            <div className="market-surface-strong p-6 md:p-8 mb-12 animate-fade-in">
              <div className="grid gap-5 lg:grid-cols-[1.1fr_0.9fr] lg:items-center">
                <div>
                  <div className="signal-kicker">Demand creation</div>
                  <h1 className="text-4xl md:text-5xl font-bold mb-4">
                    Submit <span className="text-primary">a Bounty</span>
                  </h1>
                  <p className="text-lg text-muted-foreground max-w-2xl">
                    Put demand directly onto the network. Scope the problem, set the reward, and make solvers compete for your payout.
                  </p>
                </div>
                <div className="grid gap-3 sm:grid-cols-2">
                  <div className="signal-card">
                    <div className="signal-kicker">Fast path</div>
                    <div className="mt-2 font-semibold">Choose a template, set the reward, paste the instance, and publish.</div>
                  </div>
                  <div className="signal-card">
                    <div className="signal-kicker">What matters most</div>
                    <div className="mt-2 font-semibold">Clear title, strong reward, and clean validation details pull solver attention fastest.</div>
                  </div>
                  <div className="sm:col-span-2 flex flex-col sm:flex-row gap-3">
                    <Button asChild className="sm:flex-1">
                      <Link to="/marketplace">Browse Live Market</Link>
                    </Button>
                    <Button asChild variant="outline" className="sm:flex-1">
                      <Link to="/solver-lab">Open Solver Lab</Link>
                    </Button>
                  </div>
                </div>
              </div>
            </div>

            <div className="grid gap-8 lg:grid-cols-[minmax(0,1fr)_320px] lg:items-start">
              <Card className="market-surface p-6 md:p-8 glow-primary">
                <form onSubmit={handleSubmit} className="space-y-8">
                  <section className="space-y-4">
                    <div>
                      <div className="signal-kicker">Step 1</div>
                      <h2 className="text-2xl font-semibold">Choose the market setup</h2>
                      <p className="text-sm text-muted-foreground mt-1">
                        Start with the visibility model and problem family. Everything else can be tuned afterward.
                      </p>
                    </div>

                    <div className="grid gap-3 md:grid-cols-2">
                      <button
                        type="button"
                        onClick={() => setFormData((prev) => ({ ...prev, submissionMode: "public" }))}
                        className={`rounded-2xl border p-4 text-left transition-colors ${
                          formData.submissionMode === "public"
                            ? "border-primary bg-primary/10"
                            : "border-border bg-background hover:bg-muted/50"
                        }`}
                      >
                        <div className="font-semibold">Public bounty</div>
                        <div className="mt-1 text-sm text-muted-foreground">
                          Solvers can inspect the problem immediately and begin competing right away.
                        </div>
                      </button>
                      <button
                        type="button"
                        onClick={() => setFormData((prev) => ({ ...prev, submissionMode: "private" }))}
                        className={`rounded-2xl border p-4 text-left transition-colors ${
                          formData.submissionMode === "private"
                            ? "border-primary bg-primary/10"
                            : "border-border bg-background hover:bg-muted/50"
                        }`}
                      >
                        <div className="font-semibold">Private commitment</div>
                        <div className="mt-1 text-sm text-muted-foreground">
                          Hide the full problem until reveal while still preparing escrow and market visibility.
                        </div>
                      </button>
                    </div>

                    <div className="grid gap-3 md:grid-cols-3">
                      {problemTypeOptions.map((option) => (
                        <button
                          key={option.value}
                          type="button"
                          onClick={() => setFormData((prev) => ({ ...prev, problemType: option.value }))}
                          className={`rounded-2xl border p-4 text-left transition-colors ${
                            formData.problemType === option.value
                              ? "border-primary bg-primary/10"
                              : "border-border bg-background hover:bg-muted/50"
                          }`}
                        >
                          <div className="font-semibold">{option.label}</div>
                          <div className="mt-1 text-sm text-muted-foreground">{option.note}</div>
                        </button>
                      ))}
                    </div>

                    <div className="space-y-3 rounded-2xl border border-dashed border-border/70 bg-muted/20 p-4">
                      <div>
                        <div className="font-medium">Start from an example</div>
                        <div className="text-sm text-muted-foreground mt-1">
                          Pick a real-world or starter template for {kindMeta?.label ?? formData.problemType}. Instance JSON and solver briefing load separately — no copy-paste hunting.
                        </div>
                      </div>
                      <div className="flex flex-wrap gap-2">
                        {kindTemplates.map((template) => (
                          <Button
                            key={template.id}
                            type="button"
                            variant="outline"
                            size="sm"
                            onClick={() => loadTemplate(template.id)}
                          >
                            {template.label}
                            <span className="ml-1.5 text-muted-foreground">· {template.tagline}</span>
                          </Button>
                        ))}
                      </div>
                    </div>

                    <div className="rounded-2xl border border-border/70 bg-muted/20 p-4 text-sm text-muted-foreground">
                      Live publish path: wallet-backed public and private marketplace submissions are supported here. Private bounties stay hidden until you reveal them with the matching salt and problem JSON.
                    </div>
                  </section>

                  <section className="space-y-4">
                    <div>
                      <div className="signal-kicker">Step 2</div>
                      <h2 className="text-2xl font-semibold">Define the problem</h2>
                      <p className="text-sm text-muted-foreground mt-1">
                        Instance JSON is what the chain verifies. The briefing is optional context for solvers (human-readable).
                      </p>
                    </div>

                    <div className="space-y-2">
                      <Label htmlFor="title">Problem Title</Label>
                      <Input
                        id="title"
                        value={formData.title}
                        onChange={(e) => setFormData({ ...formData, title: e.target.value })}
                        placeholder="e.g., US cold-chain vaccine relay — 15 hubs"
                        required
                      />
                    </div>

                    <div className="space-y-2">
                      <div className="flex flex-wrap items-center justify-between gap-2">
                        <Label htmlFor="instanceJson">On-chain instance JSON</Label>
                        <span
                          className={`text-xs font-medium ${
                            instancePreview.ok
                              ? "text-emerald-600 dark:text-emerald-400"
                              : formData.instanceJson.trim()
                                ? "text-destructive"
                                : "text-muted-foreground"
                          }`}
                        >
                          {instancePreview.ok
                            ? `Valid · ${instancePreview.summary}`
                            : formData.instanceJson.trim()
                              ? instancePreview.error
                              : "Required for submission"}
                        </span>
                      </div>
                      <Textarea
                        id="instanceJson"
                        value={formData.instanceJson}
                        onChange={(e) => setFormData({ ...formData, instanceJson: e.target.value })}
                        placeholder='{ "cities": 5, "distances": [[0,10,...], ...] }'
                        className="min-h-[200px] font-mono text-xs"
                        spellCheck={false}
                      />
                      <p className="text-xs text-muted-foreground">
                        {kindMeta?.instanceHint ?? "Flat JSON or wrapped `{ \"TSP\": { … } }` — this is what gets verified on-chain."}
                      </p>
                    </div>

                    <div className="space-y-2">
                      <div className="flex items-center justify-between gap-3">
                        <Label htmlFor="briefing">Solver briefing (optional)</Label>
                        <span className="text-xs text-muted-foreground">Context for humans — not stored on-chain today</span>
                      </div>
                      <Textarea
                        id="briefing"
                        value={formData.briefing}
                        onChange={(e) => setFormData({ ...formData, briefing: e.target.value })}
                        placeholder="Business context, acceptance criteria, output format notes for solvers…"
                        className="min-h-[140px] text-sm"
                      />
                    </div>
                  </section>

                  <section className="space-y-4">
                    <div>
                      <div className="signal-kicker">Step 3</div>
                      <h2 className="text-2xl font-semibold">Set reward and urgency</h2>
                      <p className="text-sm text-muted-foreground mt-1">
                        These settings shape solver behavior more than any cosmetic detail.
                      </p>
                    </div>

                    <div className="grid gap-6 md:grid-cols-2">
                      <div className="space-y-3">
                        <Label htmlFor="bounty">Bounty Amount (BEANS)</Label>
                        <Input
                          id="bounty"
                          type="number"
                          value={formData.bounty}
                          onChange={(e) => setFormData({ ...formData, bounty: e.target.value })}
                          placeholder="50000"
                          min="1"
                          required
                        />
                        <div className="flex flex-wrap gap-2">
                          {rewardPresets.map((preset) => (
                            <Button
                              key={preset}
                              type="button"
                              variant={formData.bounty === preset ? "default" : "outline"}
                              size="sm"
                              onClick={() => setFormData((prev) => ({ ...prev, bounty: preset }))}
                            >
                              {Number(preset).toLocaleString()} BEANS
                            </Button>
                          ))}
                        </div>
                        <p className="text-xs text-muted-foreground">
                          Minimum funding is 1 BEANS plus a small network fee ({formatBeans(MARKETPLACE_SUBMIT_FEE_ATOMS)} BEANS) when posting on-chain via paid paths.
                        </p>
                      </div>

                      <div className="space-y-3">
                        <Label htmlFor="expirationDays">Expiration (Days)</Label>
                        <Input
                          id="expirationDays"
                          type="number"
                          value={formData.expirationDays}
                          onChange={(e) => setFormData({ ...formData, expirationDays: e.target.value })}
                          placeholder="30"
                          min="1"
                          max="365"
                          required
                        />
                        <div className="flex flex-wrap gap-2">
                          {durationPresets.map((preset) => (
                            <Button
                              key={preset}
                              type="button"
                              variant={formData.expirationDays === preset ? "default" : "outline"}
                              size="sm"
                              onClick={() => setFormData((prev) => ({ ...prev, expirationDays: preset }))}
                            >
                              {preset} days
                            </Button>
                          ))}
                        </div>
                        <p className="text-xs text-muted-foreground">Unsolved bounties automatically refund when this window closes.</p>
                      </div>
                    </div>

                    <div className="grid gap-6 md:grid-cols-2">
                      <div className="space-y-2">
                        <Label htmlFor="minWorkScore">Minimum Work Score</Label>
                        <Input
                          id="minWorkScore"
                          type="number"
                          value={formData.minWorkScore}
                          onChange={(e) => setFormData({ ...formData, minWorkScore: e.target.value })}
                          placeholder="100"
                          min="1"
                          required
                        />
                        <p className="text-xs text-muted-foreground">Set the minimum quality threshold a solver must clear.</p>
                      </div>

                      <div className="space-y-3">
                        <Label>Complexity</Label>
                        <div className="flex flex-wrap gap-2">
                          {complexityOptions.map((option) => (
                            <Button
                              key={option.value}
                              type="button"
                              variant={formData.complexity === option.value ? "default" : "outline"}
                              size="sm"
                              onClick={() => setFormData((prev) => ({ ...prev, complexity: option.value }))}
                            >
                              {option.label}
                            </Button>
                          ))}
                        </div>
                      </div>
                    </div>
                  </section>

                  {submitError ? (
                    <div className="rounded-2xl border border-destructive/40 bg-destructive/10 p-4 text-sm text-destructive">
                      {submitError}
                    </div>
                  ) : null}

                  {confirmedSubmission ? (
                    <div className="rounded-2xl border border-primary/40 bg-primary/10 p-5">
                      <div className="signal-kicker">Confirmed creation</div>
                      <h3 className="mt-2 text-xl font-semibold">Bounty is live in the marketplace</h3>
                      <div className="mt-4 space-y-2 text-sm text-muted-foreground">
                        <p><span className="font-semibold text-foreground">Title:</span> {confirmedSubmission.title}</p>
                        <p><span className="font-semibold text-foreground">Mode:</span> {confirmedSubmission.mode === "private" ? "Private commitment" : "Public bounty"}</p>
                        <p><span className="font-semibold text-foreground">Problem ID:</span> <span className="font-mono">{confirmedSubmission.problemId}</span></p>
                        <p><span className="font-semibold text-foreground">Escrow locked:</span> {confirmedSubmission.bounty.toLocaleString()} BEANS</p>
                        <p><span className="font-semibold text-foreground">Submitter:</span> <span className="font-mono">{confirmedSubmission.submitter}</span></p>
                      </div>
                      <div className="mt-4 flex flex-wrap gap-2">
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          onClick={() => copyToClipboard(confirmedSubmission.problemId, "Problem ID")}
                        >
                          {copiedField === "Problem ID" ? "Copied problem ID" : "Copy problem ID"}
                        </Button>
                        {confirmedSubmission.commitment ? (
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            onClick={() => copyToClipboard(confirmedSubmission.commitment!, "Commitment")}
                          >
                            {copiedField === "Commitment" ? "Copied commitment" : "Copy commitment"}
                          </Button>
                        ) : null}
                      </div>
                      {confirmedSubmission.mode === "private" ? (
                        <div className="mt-5 rounded-2xl border border-amber-500/40 bg-amber-500/10 p-4">
                          <div className="signal-kicker">Save your reveal kit</div>
                          <p className="mt-2 text-sm text-muted-foreground">
                            This salt and exact problem JSON are required later for `marketplace_revealProblem`. If you lose either one, you cannot reveal the private bounty correctly.
                          </p>
                          <div className="mt-4 space-y-4">
                            <div className="space-y-2">
                              <div className="flex items-center justify-between gap-3">
                                <span className="text-sm font-semibold text-foreground">Salt</span>
                                <Button
                                  type="button"
                                  variant="outline"
                                  size="sm"
                                  onClick={() => copyToClipboard(confirmedSubmission.salt ?? "", "Salt")}
                                >
                                  {copiedField === "Salt" ? "Copied salt" : "Copy salt"}
                                </Button>
                              </div>
                              <div className="rounded-xl bg-background/80 p-3 font-mono text-xs break-all">
                                {confirmedSubmission.salt}
                              </div>
                            </div>
                            <div className="space-y-2">
                              <div className="flex items-center justify-between gap-3">
                                <span className="text-sm font-semibold text-foreground">Problem JSON</span>
                                <Button
                                  type="button"
                                  variant="outline"
                                  size="sm"
                                  onClick={() => copyToClipboard(confirmedSubmission.problemJson, "Problem JSON")}
                                >
                                  {copiedField === "Problem JSON" ? "Copied problem JSON" : "Copy problem JSON"}
                                </Button>
                              </div>
                              <pre className="max-h-56 overflow-auto rounded-xl bg-background/80 p-3 text-xs text-foreground whitespace-pre-wrap break-all">
                                {confirmedSubmission.problemJson}
                              </pre>
                            </div>
                          </div>
                        </div>
                      ) : null}
                      <div className="mt-4 flex flex-col gap-3 sm:flex-row">
                        <Button asChild className="sm:flex-1">
                          <Link to="/marketplace">View live market</Link>
                        </Button>
                        <Button asChild variant="outline" className="sm:flex-1">
                          <Link to="/wallet">Review wallet</Link>
                        </Button>
                      </div>
                    </div>
                  ) : null}

                  <details className="rounded-2xl border border-border/70 bg-muted/20 p-5">
                    <summary className="cursor-pointer list-none font-semibold">
                      Advanced options
                    </summary>
                    <div className="mt-5 grid gap-6">
                      <div className="grid gap-6 md:grid-cols-3">
                        <div className="space-y-2">
                          <Label htmlFor="priority">Priority</Label>
                          <select
                            id="priority"
                            value={formData.priority}
                            onChange={(e) => setFormData({ ...formData, priority: e.target.value })}
                            className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                          >
                            {priorityOptions.map((option) => (
                              <option key={option.value} value={option.value}>
                                {option.label}
                              </option>
                            ))}
                          </select>
                        </div>

                        <div className="space-y-2">
                          <Label htmlFor="verificationMethod">Verification Method</Label>
                          <select
                            id="verificationMethod"
                            value={formData.verificationMethod}
                            onChange={(e) => setFormData({ ...formData, verificationMethod: e.target.value })}
                            className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                          >
                            <option value="automated">Automated Testing</option>
                            <option value="manual">Manual Review</option>
                            <option value="hybrid">Hybrid Verification</option>
                            <option value="community">Community Voting</option>
                          </select>
                        </div>

                        <div className="space-y-2">
                          <Label htmlFor="aggregationMethod">Aggregation Method</Label>
                          <select
                            id="aggregationMethod"
                            value={formData.aggregationMethod}
                            onChange={(e) => setFormData({ ...formData, aggregationMethod: e.target.value })}
                            className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                          >
                            <option value="best_solution">Best Solution</option>
                            <option value="first_valid">First Valid</option>
                            <option value="consensus">Consensus</option>
                            <option value="weighted_average">Weighted Average</option>
                          </select>
                        </div>
                      </div>

                      <div className="space-y-2">
                        <Label htmlFor="notes">Additional Notes</Label>
                        <Textarea
                          id="notes"
                          value={formData.notes}
                          onChange={(e) => setFormData({ ...formData, notes: e.target.value })}
                          placeholder="Anything special solvers should know before they commit time."
                          className="min-h-[100px]"
                        />
                      </div>
                    </div>
                  </details>

                  <div className="rounded-2xl border border-border/70 bg-muted/20 p-4 text-sm text-muted-foreground">
                    Escrow locks immediately on submission. Valid solutions pay out automatically, and unsolved work refunds after expiry.
                  </div>

                  <Button type="submit" className="w-full" size="lg" disabled={!canSubmit}>
                    {isSubmitting
                      ? "Submitting to chain..."
                      : !instancePreview.ok
                        ? "Fix instance JSON to submit"
                        : !hasSufficientBalance && hasWallet
                          ? "Insufficient balance"
                          : `Submit Bounty and Escrow ${formData.bounty || "0"} BEANS`}
                  </Button>
                </form>
              </Card>

              <div className="space-y-6 lg:sticky lg:top-28">
                <Card className="market-surface p-6">
                  <div className="signal-kicker">Submission summary</div>
                  <h3 className="mt-2 text-xl font-semibold">Ready to publish?</h3>
                  <div className="mt-5 space-y-3 rounded-2xl border border-border/70 bg-muted/20 p-4">
                    <ChecklistRow
                      ok={hasWallet ? true : false}
                      label="Wallet connected"
                      detail={hasWallet ? selectedKeyPair!.address.slice(0, 10) + "…" : "Open Wallet and create or select an account"}
                    />
                    <ChecklistRow ok={hasTitle ? true : false} label="Title set" />
                    <ChecklistRow
                      ok={instancePreview.ok ? true : formData.instanceJson.trim() ? false : null}
                      label="Instance JSON valid"
                      detail={
                        instancePreview.ok
                          ? instancePreview.summary
                          : formData.instanceJson.trim()
                            ? instancePreview.error
                            : "Load an example or paste JSON"
                      }
                    />
                    <ChecklistRow
                      ok={
                        !hasWallet || !hasValidBounty
                          ? null
                          : walletBalance === undefined
                            ? null
                            : walletBalance >= totalEscrowAtoms
                      }
                      label="Escrow funded"
                      detail={
                        hasValidBounty
                          ? `Need ${formatBeans(totalEscrowAtoms)} BEANS total`
                          : "Set bounty amount"
                      }
                    />
                  </div>
                  <div className="mt-5 space-y-4">
                    <div className="flex items-center justify-between gap-4 text-sm">
                      <span className="text-muted-foreground">Wallet</span>
                      <span className="font-semibold">
                        {selectedAccount ?? "Not connected"}
                      </span>
                    </div>
                    <div className="flex items-center justify-between gap-4 text-sm">
                      <span className="text-muted-foreground">Available balance</span>
                      <span className="font-semibold">
                        {walletBalance !== undefined
                          ? `${formatBeans(walletBalance)} BEANS`
                          : "Connect wallet"}
                      </span>
                    </div>
                    <div className="flex items-center justify-between gap-4 text-sm">
                      <span className="text-muted-foreground">Mode</span>
                      <span className="font-semibold capitalize">{formData.submissionMode}</span>
                    </div>
                    <div className="flex items-center justify-between gap-4 text-sm">
                      <span className="text-muted-foreground">Problem type</span>
                      <span className="font-semibold">{selectedProblemType?.label ?? formData.problemType}</span>
                    </div>
                    <div className="flex items-center justify-between gap-4 text-sm">
                      <span className="text-muted-foreground">Reward</span>
                      <span className="font-semibold">{Number(formData.bounty || 0).toLocaleString()} BEANS</span>
                    </div>
                    <div className="flex items-center justify-between gap-4 text-sm">
                      <span className="text-muted-foreground">Network fee</span>
                      <span className="font-semibold">{formatBeans(MARKETPLACE_SUBMIT_FEE_ATOMS)} BEANS</span>
                    </div>
                    <div className="flex items-center justify-between gap-4 text-sm">
                      <span className="text-muted-foreground">Expires in</span>
                      <span className="font-semibold">{formData.expirationDays} days</span>
                    </div>
                    <div className="h-px bg-border" />
                    <div className="flex items-center justify-between gap-4">
                      <span className="font-semibold text-primary">Total required</span>
                      <span className="text-xl font-bold text-primary">{formatBeans(totalEscrowAtoms)} BEANS</span>
                    </div>
                  </div>
                </Card>
                <Card className="market-surface p-6">
                  <div className="signal-kicker">Operator notes</div>
                  <div className="mt-2 space-y-3 text-sm text-muted-foreground">
                    <p>Keep titles concrete. Solver interest rises when the outcome is obvious before opening the full brief.</p>
                    <p>Use higher rewards and shorter expiry when you need immediate attention.</p>
                    <p>Private bounties need a later reveal step, so keep the salt and exact problem JSON paired together.</p>
                  </div>
                </Card>
                <Card className="market-surface p-6">
                  <div className="signal-kicker">Reveal private bounty</div>
                  <h3 className="mt-2 text-xl font-semibold">Manage the reveal here</h3>
                  <p className="mt-2 text-sm text-muted-foreground">
                    When you are ready to open a private bounty to solvers, submit the original problem JSON and matching salt from the same screen.
                  </p>
                  <div className="mt-5">
                    <div className="mb-3 flex items-center justify-between gap-3">
                      <div className="text-sm font-semibold">My private bounties</div>
                      <span className="text-xs text-muted-foreground">
                        {selectedKeyPair?.address
                          ? `${myPrivateBounties.length} unrevealed`
                          : "Connect wallet"}
                      </span>
                    </div>
                    {selectedKeyPair?.address ? (
                      myPrivateBounties.length > 0 ? (
                        <div className="space-y-2">
                          {myPrivateBounties.slice(0, 5).map((problem) => (
                            (() => {
                              const savedKit = selectedKeyPair?.address
                                ? storedRevealKits[revealKitKey(selectedKeyPair.address, problem.problem_id)]
                                : undefined;
                              const displayTitle = savedKit?.title || problem.problem_type || "Private bounty";

                              return (
                                <button
                                  key={problem.problem_id}
                                  type="button"
                                  onClick={() => loadRevealKitIntoForm(problem.problem_id)}
                                  className="w-full rounded-2xl border border-border/70 bg-background/60 p-3 text-left transition-colors hover:bg-muted/60"
                                >
                                  <div className="flex items-start justify-between gap-3">
                                    <div className="min-w-0">
                                      <div className="truncate text-sm font-semibold text-foreground">
                                        {displayTitle}
                                      </div>
                                      <div className="mt-1 font-mono text-xs text-muted-foreground">
                                        {problem.problem_id.slice(0, 18)}...{problem.problem_id.slice(-8)}
                                      </div>
                                    </div>
                                    <span className="shrink-0 text-xs font-semibold text-primary">
                                      {problem.problem_type ?? "Private"}
                                    </span>
                                  </div>
                                  <div className="mt-2 flex items-center justify-between gap-3 text-xs text-muted-foreground">
                                    <span>
                                      {formatBeans(parseBalance(problem.bounty) ?? 0n)} BEANS
                                    </span>
                                    <span>
                                      {savedKit
                                        ? "kit saved"
                                        : new Date(problem.expires_at * 1000).toLocaleDateString()}
                                    </span>
                                  </div>
                                </button>
                              );
                            })()
                          ))}
                        </div>
                      ) : (
                        <div className="rounded-2xl border border-border/70 bg-muted/20 p-4 text-sm text-muted-foreground">
                          No unrevealed private bounties found for this wallet right now.
                        </div>
                      )
                    ) : (
                      <div className="rounded-2xl border border-border/70 bg-muted/20 p-4 text-sm text-muted-foreground">
                        Connect a wallet to load your unrevealed private bounties.
                      </div>
                    )}
                  </div>
                  <form onSubmit={handleReveal} className="mt-5 space-y-4">
                    <div className="space-y-2">
                      <Label htmlFor="reveal-problem-id">Problem ID</Label>
                      <Input
                        id="reveal-problem-id"
                        value={revealForm.problemId}
                        onChange={(e) => setRevealForm((prev) => ({ ...prev, problemId: e.target.value }))}
                        placeholder="Hex problem ID"
                      />
                    </div>
                    <div className="space-y-2">
                      <div className="flex items-center justify-between gap-3">
                        <Label htmlFor="reveal-salt">Reveal Salt</Label>
                        {confirmedSubmission?.mode === "private" && confirmedSubmission.salt ? (
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            onClick={() => setRevealForm((prev) => ({ ...prev, salt: confirmedSubmission.salt ?? prev.salt }))}
                          >
                            Use latest private salt
                          </Button>
                        ) : null}
                      </div>
                      <Input
                        id="reveal-salt"
                        value={revealForm.salt}
                        onChange={(e) => setRevealForm((prev) => ({ ...prev, salt: e.target.value }))}
                        placeholder="0x..."
                      />
                    </div>
                    <div className="space-y-2">
                      <div className="flex items-center justify-between gap-3">
                        <Label htmlFor="reveal-problem-json">Problem JSON</Label>
                        {confirmedSubmission?.mode === "private" ? (
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            onClick={() => setRevealForm((prev) => ({
                              ...prev,
                              problemJson: confirmedSubmission.problemJson,
                              problemId: confirmedSubmission.problemId,
                              salt: confirmedSubmission.salt ?? prev.salt,
                            }))}
                          >
                            Load latest private payload
                          </Button>
                        ) : null}
                      </div>
                      <Textarea
                        id="reveal-problem-json"
                        value={revealForm.problemJson}
                        onChange={(e) => setRevealForm((prev) => ({ ...prev, problemJson: e.target.value }))}
                        placeholder='{"SubsetSum":{"numbers":[3,34,4,12,5,2],"target":15}}'
                        className="min-h-[180px] font-mono text-xs"
                      />
                    </div>
                    {revealError ? (
                      <div className="rounded-2xl border border-destructive/40 bg-destructive/10 p-4 text-sm text-destructive">
                        {revealError}
                      </div>
                    ) : null}
                    {revealedProblemId ? (
                      <div className="rounded-2xl border border-primary/40 bg-primary/10 p-4 text-sm text-muted-foreground">
                        <span className="font-semibold text-foreground">Reveal confirmed:</span> <span className="font-mono">{revealedProblemId}</span> is now visible to solvers.
                      </div>
                    ) : null}
                    <Button type="submit" className="w-full" disabled={isRevealing}>
                      {isRevealing ? "Revealing on-chain..." : "Reveal Private Bounty"}
                    </Button>
                  </form>
                </Card>
              </div>
            </div>

            <div className="mt-8 grid md:grid-cols-3 gap-4">
              <Card className="p-4 text-center">
                <div className="text-3xl font-bold text-primary mb-1">
                  {typeof marketplaceStats?.open_problems === "number"
                    ? marketplaceStats.open_problems.toLocaleString()
                    : "Live"}
                </div>
                <div className="text-sm text-muted-foreground">Active Problems</div>
              </Card>
              <Card className="p-4 text-center">
                <div className="text-3xl font-bold text-primary mb-1">
                  {marketplaceStats?.total_bounty_pool != null
                    ? `${formatBeans(parseBalance(marketplaceStats.total_bounty_pool) ?? 0n)}`
                    : "Live"}
                </div>
                <div className="text-sm text-muted-foreground">Total BEANS Escrowed</div>
              </Card>
              <Card className="p-4 text-center">
                <div className="text-3xl font-bold text-primary mb-1">
                  {typeof marketplaceStats?.solved_problems === "number"
                    ? marketplaceStats.solved_problems.toLocaleString()
                    : "Live"}
                </div>
                <div className="text-sm text-muted-foreground">Solved Problems</div>
              </Card>
            </div>
          </div>
        </div>
      </main>
      <Footer />
    </div>
  );
};

export default BountySubmit;
