import { Navigation } from "@/components/Navigation";
import { Footer } from "@/components/Footer";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { useToast } from "@/hooks/use-toast";
import { useState, useEffect } from "react";
import { Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import type { ProblemType } from "@/lib/rpc-client";
import { rpcClient } from "@/lib/rpc-client";
import { useWallet } from "@/contexts/WalletContext";

/** Must match `STORAGE_KEY` in `NpPlayground.tsx` (Solver Lab → Bounty draft). */
const SOLVER_LAB_BOUNTY_KEY = "solverLabBountyPayload";
const TRANSACTION_FEE = 1000;

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
  problemType: "SubsetSum",
  description: "",
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

function extractCandidateJson(description: string): string | null {
  const fencedMatch = description.match(/```(?:json)?\s*([\s\S]*?)```/i);
  if (fencedMatch?.[1]) {
    return fencedMatch[1].trim();
  }

  const firstBrace = description.indexOf("{");
  const lastBrace = description.lastIndexOf("}");
  if (firstBrace >= 0 && lastBrace > firstBrace) {
    return description.slice(firstBrace, lastBrace + 1).trim();
  }

  return null;
}

function parseProblemFromDescription(
  description: string,
  problemType: string,
): ProblemType {
  const candidateJson = extractCandidateJson(description);
  if (!candidateJson) {
    throw new Error(`Paste a ${problemType} instance JSON block into the description before submitting.`);
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(candidateJson);
  } catch {
    throw new Error(`The problem description must include valid JSON for the live ${problemType} instance.`);
  }

  const record = parsed as Record<string, unknown>;
  if (problemType === "SubsetSum") {
    if (!Array.isArray(record.numbers) || typeof record.target !== "number") {
      throw new Error("Subset Sum submissions require JSON with `numbers` and `target` fields.");
    }

    const numbers = record.numbers.map((value) => {
      if (typeof value !== "number" || !Number.isFinite(value)) {
        throw new Error("Each Subset Sum number must be a valid integer.");
      }
      return Math.trunc(value);
    });

    if (numbers.length === 0) {
      throw new Error("Subset Sum submissions require at least one input number.");
    }

    return {
      SubsetSum: {
        numbers,
        target: Math.trunc(record.target),
      },
    };
  }

  if (problemType === "SAT") {
    if (typeof record.variables !== "number" || !Array.isArray(record.clauses)) {
      throw new Error("SAT submissions require JSON with `variables` and `clauses` fields.");
    }

    const clauses = record.clauses.map((clause) => {
      const literals = Array.isArray(clause)
        ? clause
        : typeof clause === "object" && clause !== null && Array.isArray((clause as { literals?: unknown }).literals)
          ? (clause as { literals: unknown[] }).literals
          : null;

      if (!literals) {
        throw new Error("Each SAT clause must be an array of integers or an object with `literals`.");
      }

      return {
        literals: literals.map((literal) => {
          if (typeof literal !== "number" || !Number.isFinite(literal)) {
            throw new Error("SAT literals must be valid integers.");
          }
          return Math.trunc(literal);
        }),
      };
    });

    return {
      SAT: {
        variables: Math.trunc(record.variables),
        clauses,
      },
    };
  }

  if (problemType === "TSP") {
    if (typeof record.cities !== "number" || !Array.isArray(record.distances)) {
      throw new Error("TSP submissions require JSON with `cities` and `distances` fields.");
    }

    const distances = record.distances.map((row) => {
      if (!Array.isArray(row)) {
        throw new Error("Each TSP distance row must be an array of integers.");
      }

      return row.map((value) => {
        if (typeof value !== "number" || !Number.isFinite(value)) {
          throw new Error("TSP distances must be valid integers.");
        }
        return Math.max(0, Math.trunc(value));
      });
    });

    return {
      TSP: {
        cities: Math.trunc(record.cities),
        distances,
      },
    };
  }

  throw new Error(`Unsupported problem type: ${problemType}`);
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

  const totalRequired = formData.bounty ? parseInt(formData.bounty) + TRANSACTION_FEE : TRANSACTION_FEE;
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
  const problemTypeOptions = [
    { value: "SubsetSum", label: "Subset Sum", note: "Good for exact search and matching problems." },
    { value: "TSP", label: "TSP", note: "Best for routing and optimization style work." },
    { value: "SAT", label: "SAT", note: "Best for satisfiability and constraint-heavy work." },
  ];
  const rewardPresets = ["25000", "50000", "100000", "250000"];
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
      problem.status === "Open"
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
        draftKind?: "problem" | "solver";
      };
      sessionStorage.removeItem(SOLVER_LAB_BOUNTY_KEY);
      if (data.title && data.description) {
        setFormData((prev) => ({
          ...prev,
          title: data.title!,
          description: data.description!,
          problemType: data.problemType ?? prev.problemType,
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

  // Problem templates with example data
  const problemTemplates = {
    SubsetSum: {
      title: "Large Dataset Subset Sum Challenge",
      description: `**Problem:** Subset Sum (NP-Complete)

**Input Format:**
- Target sum: T (integer)
- Array of integers: [a₁, a₂, ..., aₙ]
- Array size: n elements

**Example Input:**
\`\`\`json
{
  "target": 15,
  "numbers": [3, 34, 4, 12, 5, 2],
  "size": 6
}
\`\`\`

**Expected Output:**
Return a subset of numbers that sum exactly to the target.

**Example Output:**
\`\`\`json
{
  "solution": [3, 12],
  "sum": 15,
  "indices": [0, 3]
}
\`\`\`

**Constraints:**
- 1 ≤ n ≤ 1000
- -10⁶ ≤ aᵢ ≤ 10⁶
- Solution must be exact (not approximate)

**Verification:** Automated - sum of returned subset must equal target`,
      bounty: "50000",
      minWorkScore: "150",
      complexity: "medium"
    },
    TSP: {
      title: "Traveling Salesman Route Optimization",
      description: `**Problem:** Traveling Salesman Problem (NP-Complete)

**Input Format:**
- Number of cities: n
- Distance matrix: n×n matrix where d[i][j] = distance from city i to city j
- Starting city: city_id

**Example Input:**
\`\`\`json
{
  "cities": 5,
  "distances": [
    [0, 10, 15, 20, 25],
    [10, 0, 35, 25, 30],
    [15, 35, 0, 30, 20],
    [20, 25, 30, 0, 15],
    [25, 30, 20, 15, 0]
  ],
  "start_city": 0
}
\`\`\`

**Expected Output:**
Return the shortest tour visiting all cities exactly once and returning to start.

**Example Output:**
\`\`\`json
{
  "tour": [0, 1, 3, 4, 2, 0],
  "total_distance": 95,
  "visited_all": true
}
\`\`\`

**Constraints:**
- 3 ≤ n ≤ 100
- All distances are positive integers
- Triangle inequality may or may not hold
- Must return to starting city

**Verification:** Automated - validate tour completeness and distance calculation`,
      bounty: "100000",
      minWorkScore: "200",
      complexity: "hard"
    },
    SAT: {
      title: "3-SAT Boolean Satisfiability Instance",
      description: `**Problem:** Boolean Satisfiability (SAT/3-SAT) - NP-Complete

**Input Format:**
- Variables: set of boolean variables {x₁, x₂, ..., xₙ}
- Clauses: CNF formula with clauses of up to 3 literals each
- Number of clauses: m

**Example Input:**
\`\`\`json
{
  "variables": 4,
  "clauses": [
    [1, -2, 3],    // (x₁ ∨ ¬x₂ ∨ x₃)
    [-1, 2, -3],   // (¬x₁ ∨ x₂ ∨ ¬x₃)
    [2, 3, 4],     // (x₂ ∨ x₃ ∨ x₄)
    [-2, -3, 4]    // (¬x₂ ∨ ¬x₃ ∨ x₄)
  ]
}
\`\`\`
Note: Positive numbers = variable, negative = negation

**Expected Output:**
Return a satisfying assignment or proof of unsatisfiability.

**Example Output:**
\`\`\`json
{
  "satisfiable": true,
  "assignment": {
    "x1": true,
    "x2": false,
    "x3": true,
    "x4": true
  },
  "verified_clauses": 4
}
\`\`\`

**Constraints:**
- 3 ≤ n ≤ 500 variables
- Each clause has ≤ 3 literals
- CNF (Conjunctive Normal Form) only
- Must satisfy ALL clauses

**Verification:** Automated - evaluate assignment against all clauses`,
      bounty: "75000",
      minWorkScore: "180",
      complexity: "expert"
    }
  };

  const loadTemplate = (problemType: string) => {
    const template = problemTemplates[problemType as keyof typeof problemTemplates];
    if (template) {
      setFormData({
        ...formData,
        title: template.title,
        description: template.description,
        bounty: template.bounty,
        minWorkScore: template.minWorkScore,
        complexity: template.complexity,
        problemType: problemType
      });
      
      toast({
        title: "Template Loaded ✨",
        description: `${problemType} example loaded. Customize as needed.`,
      });
    }
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

    if (!Number.isFinite(bounty) || bounty < 1000) {
      setSubmitError("Bounty must be at least 1,000 BEANS.");
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

    if (typeof walletBalance === "number" && walletBalance < bounty) {
      const message = `Insufficient wallet balance. Available: ${walletBalance.toLocaleString()} BEANS, required escrow: ${bounty.toLocaleString()} BEANS.`;
      setSubmitError(message);
      toast({ title: "Insufficient balance", description: message, variant: "destructive" });
      return;
    }

    let parsedProblem: ProblemType;
    try {
      parsedProblem = parseProblemFromDescription(formData.description, formData.problemType);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to parse the problem instance.";
      setSubmitError(message);
      toast({ title: "Invalid problem JSON", description: message, variant: "destructive" });
      return;
    }

    setIsSubmitting(true);

    try {
      let problemId: string;
      let commitment: string | undefined;
      let salt: string | undefined;

      if (formData.submissionMode === "private") {
        salt = generateSaltHex();
        const privateResult = await rpcClient.submitPrivateProblemWithWallet({
          problem: parsedProblem,
          salt,
          bounty,
          min_work_score: minWorkScore,
          expiration_days: expirationDays,
          submitter: selectedKeyPair.address,
        });
        problemId = privateResult.problem_id;
        commitment = privateResult.commitment;
      } else {
        problemId = await rpcClient.submitPublicProblem({
          problem: parsedProblem,
          bounty,
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

                    <div className="flex flex-col gap-3 rounded-2xl border border-dashed border-border/70 bg-muted/20 p-4 md:flex-row md:items-center md:justify-between">
                      <div>
                        <div className="font-medium">Need a starting point?</div>
                        <div className="text-sm text-muted-foreground">
                          Load a ready-made {selectedProblemType?.label ?? formData.problemType} example, then edit only what matters.
                        </div>
                      </div>
                      <Button type="button" variant="outline" onClick={() => loadTemplate(formData.problemType)}>
                        Load Example
                      </Button>
                    </div>

                    <div className="rounded-2xl border border-border/70 bg-muted/20 p-4 text-sm text-muted-foreground">
                      Live publish path: wallet-backed public and private marketplace submissions are supported here. Private bounties stay hidden until you reveal them with the matching salt and problem JSON.
                    </div>
                  </section>

                  <section className="space-y-4">
                    <div>
                      <div className="signal-kicker">Step 2</div>
                      <h2 className="text-2xl font-semibold">Describe the work clearly</h2>
                      <p className="text-sm text-muted-foreground mt-1">
                        Keep this operator-friendly. A sharp prompt gets better submissions than a long wall of text.
                      </p>
                    </div>

                    <div className="space-y-2">
                      <Label htmlFor="title">Problem Title</Label>
                      <Input
                        id="title"
                        value={formData.title}
                        onChange={(e) => setFormData({ ...formData, title: e.target.value })}
                        placeholder="e.g., Optimize route coverage for 5,000 delivery nodes"
                        required
                      />
                    </div>

                    <div className="space-y-2">
                      <div className="flex items-center justify-between gap-3">
                        <Label htmlFor="description">Problem Description</Label>
                        <span className="text-xs text-muted-foreground">Include input format, expected output, and verification rules.</span>
                      </div>
                      <Textarea
                        id="description"
                        value={formData.description}
                        onChange={(e) => setFormData({ ...formData, description: e.target.value })}
                        placeholder="Paste the instance JSON or describe the exact format solvers should target. Make acceptance criteria explicit."
                        className="min-h-[220px] font-mono text-sm"
                        required
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
                          min="1000"
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
                        <p className="text-xs text-muted-foreground">Minimum funding is 1,000 BEANS plus the network fee.</p>
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

                  <Button type="submit" className="w-full" size="lg" disabled={isSubmitting}>
                    {isSubmitting
                      ? "Submitting to chain..."
                      : `Submit Bounty and Escrow ${formData.bounty || "0"} BEANS`}
                  </Button>
                </form>
              </Card>

              <div className="space-y-6 lg:sticky lg:top-28">
                <Card className="market-surface p-6">
                  <div className="signal-kicker">Submission summary</div>
                  <h3 className="mt-2 text-xl font-semibold">Ready to publish?</h3>
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
                        {typeof walletBalance === "number"
                          ? `${walletBalance.toLocaleString()} BEANS`
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
                      <span className="font-semibold">{TRANSACTION_FEE.toLocaleString()} BEANS</span>
                    </div>
                    <div className="flex items-center justify-between gap-4 text-sm">
                      <span className="text-muted-foreground">Expires in</span>
                      <span className="font-semibold">{formData.expirationDays} days</span>
                    </div>
                    <div className="h-px bg-border" />
                    <div className="flex items-center justify-between gap-4">
                      <span className="font-semibold text-primary">Total required</span>
                      <span className="text-xl font-bold text-primary">{totalRequired.toLocaleString()} BEANS</span>
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
                                    <span>{problem.bounty.toLocaleString()} BEANS</span>
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
                  {typeof marketplaceStats?.total_bounty_pool === "number"
                    ? `${(marketplaceStats.total_bounty_pool / 1e9).toFixed(2)}B`
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
