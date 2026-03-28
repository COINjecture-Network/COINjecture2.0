import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTheme } from "next-themes";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable";
import { WebCliTerminal } from "@/components/WebCliTerminal";
import {
  FileJson,
  FileCode2,
  Play,
  Trash2,
  Network,
  Send,
  RotateCcw,
  FolderOpen,
  Link2,
  Terminal,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { SolverRunResult } from "./types";
import {
  WORKSPACE_FILES,
  loadWorkspaceFromStorage,
  saveWorkspaceToStorage,
  resetWorkspaceDefaults,
  type WorkspaceFilePath,
} from "./defaultSolverWorkspace";
import { DEFAULT_PROBLEM_JSON } from "./defaults";
import type { NetworkProblemKind } from "./networkRegistry";
import { NETWORK_REGISTRY } from "./networkRegistry";
import { parseNetworkProblem, problemTypesEqual } from "./parseNetworkProblem";
import { SolverCodeEditor } from "./SolverCodeEditor";
import { runUserSolver } from "./userSolverRunner";
import { normalizeSolution } from "./solutionNormalize";
import { SubsetSumVisualizer } from "./visualizers/SubsetSumVisualizer";
import { TSPVisualizer } from "./visualizers/TSPVisualizer";
import { SATVisualizer } from "./visualizers/SATVisualizer";
import { useWallet } from "@/contexts/WalletContext";
import { rpcClient } from "@/lib/rpc-client";
import { createBlockFromSolvedProblem, extractHashHex, type Solution as MiningSolution } from "@/lib/mining";

/** Alias for `<Editor />` — must stay after all imports (ES modules forbid statements between imports). */
const Editor = SolverCodeEditor;

const STORAGE_KEY = "solverLabBountyPayload";

const FILE_META: Record<WorkspaceFilePath, { label: string; icon: typeof FileCode2 }> = {
  "solvers/subset-sum.js": { label: "subset-sum.js", icon: FileCode2 },
  "solvers/sat.js": { label: "sat.js", icon: FileCode2 },
  "solvers/tsp.js": { label: "tsp.js", icon: FileCode2 },
  "instance.json": { label: "instance.json", icon: FileJson },
};

type NpPlaygroundProps = {
  className?: string;
};

export function NpPlayground({ className }: NpPlaygroundProps) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { resolvedTheme } = useTheme();
  const { selectedAccount, accounts } = useWallet();
  const selectedKeyPair = selectedAccount ? accounts[selectedAccount] ?? null : null;
  const [workspace, setWorkspace] = useState<"solvers" | "chain">("solvers");
  const [files, setFiles] = useState<Record<WorkspaceFilePath, string>>(() => loadWorkspaceFromStorage());
  const [activeFile, setActiveFile] = useState<WorkspaceFilePath>("solvers/subset-sum.js");
  const [runResult, setRunResult] = useState<SolverRunResult | null>(null);
  const [consoleLines, setConsoleLines] = useState<string[]>([]);
  const [running, setRunning] = useState(false);
  const [submittingChain, setSubmittingChain] = useState(false);
  const [pullingChainInstance, setPullingChainInstance] = useState(false);
  const [isLg, setIsLg] = useState(true);
  const [mobilePanel, setMobilePanel] = useState<"code" | "visual" | "result" | "console">("code");

  useEffect(() => {
    const mq = window.matchMedia("(min-width: 1024px)");
    const apply = () => setIsLg(mq.matches);
    apply();
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, []);

  useEffect(() => {
    saveWorkspaceToStorage(files);
  }, [files]);

  const { data: chainInfo } = useQuery({
    queryKey: ["solverLab", "chainInfo"],
    queryFn: () => rpcClient.getChainInfo(),
    refetchInterval: 15_000,
    staleTime: 5_000,
  });

  const { data: miningWork } = useQuery({
    queryKey: ["solverLab", "miningWork"],
    queryFn: () => rpcClient.getMiningWork(),
    refetchInterval: 15_000,
    staleTime: 5_000,
  });

  const instanceText = files["instance.json"];

  const parsedPreview = useMemo(() => parseNetworkProblem(instanceText), [instanceText]);

  /** Problem kind from `instance.json` (what Run actually executes). */
  const instanceProblemKind = useMemo(() => {
    if (!parsedPreview.ok) return null;
    const v = parsedPreview.value;
    if (v.SubsetSum) return "SubsetSum" as const;
    if (v.SAT) return "SAT" as const;
    if (v.TSP) return "TSP" as const;
    return null;
  }, [parsedPreview]);

  const instanceMatchesMiningWork = useMemo(() => {
    if (!miningWork || !parsedPreview.ok) return false;
    return problemTypesEqual(miningWork.problem, parsedPreview.value);
  }, [miningWork, parsedPreview]);

  /** Run label reflects the open file; execution always uses all solver files + instance.json. */
  const runButtonLabel = useMemo(() => {
    if (running) return "…";
    switch (activeFile) {
      case "instance.json":
        return instanceProblemKind ? `Run ${instanceProblemKind}` : "Run";
      case "solvers/subset-sum.js":
        return "Run subset-sum";
      case "solvers/sat.js":
        return "Run SAT solver";
      case "solvers/tsp.js":
        return "Run TSP solver";
      default:
        return "Run";
    }
  }, [activeFile, running, instanceProblemKind]);

  const runButtonTitle =
    "Runs your edited workspace: all files under solvers/*.js plus instance.json. The problem type comes from instance.json.";

  /** When a solver file is open, which ProblemType that tab implies (visualization still follows instance.json). */
  const activeSolverExpectedKind: NetworkProblemKind | null = useMemo(() => {
    switch (activeFile) {
      case "solvers/subset-sum.js":
        return "SubsetSum";
      case "solvers/sat.js":
        return "SAT";
      case "solvers/tsp.js":
        return "TSP";
      default:
        return null;
    }
  }, [activeFile]);

  const instanceKindMismatch =
    activeSolverExpectedKind != null &&
    instanceProblemKind != null &&
    activeSolverExpectedKind !== instanceProblemKind;

  const setFileContent = useCallback((path: WorkspaceFilePath, value: string) => {
    setFiles((prev) => ({ ...prev, [path]: value }));
  }, []);

  const onEditorChange = useCallback(
    (value: string | undefined) => {
      setFileContent(activeFile, value ?? "");
    },
    [activeFile, setFileContent]
  );

  const run = async () => {
    const parsed = parseNetworkProblem(instanceText);
    if (!parsed.ok) {
      setRunResult(null);
      setConsoleLines((prev) => [...prev, `[error] ${parsed.error}`]);
      return;
    }
    setRunning(true);
    setRunResult(null);
    try {
      const out = await runUserSolver(files, parsed.value, 45000);
      if (!out.ok) {
        setRunResult({
          ok: false,
          timeMs: out.timeMs ?? 0,
          solution: null,
          log: [out.error],
        });
        setConsoleLines((prev) => [...prev, `[error] ${out.error}`, ""]);
        return;
      }
      const normalized = normalizeSolution(parsed.value, out.solution);
      setRunResult({
        ok: normalized != null,
        timeMs: out.timeMs,
        solution: normalized,
        log: normalized ? [] : ["Solution shape did not match ProblemType (expected SubsetSum / SAT / TSP keys)."],
      });
      setConsoleLines((prev) => [
        ...prev,
        `[solver] ${out.timeMs.toFixed(3)}ms`,
        `raw: ${JSON.stringify(out.solution)}`,
        normalized ? `normalized: ${JSON.stringify(normalized)}` : "[warn] Could not normalize — check return shape",
        "",
      ]);
    } finally {
      setRunning(false);
    }
  };

  const clearConsole = () => setConsoleLines([]);

  const resetAll = () => {
    if (!window.confirm("Reset all workspace files to defaults? Your edits will be lost.")) return;
    setFiles(resetWorkspaceDefaults());
    setRunResult(null);
    setConsoleLines(["Workspace reset to default algorithms + instance.", ""]);
  };

  /** `chain_getMiningWork` → `instance.json` (same deterministic problem as validating miners). */
  const pullLiveInstanceFromChain = async () => {
    setPullingChainInstance(true);
    try {
      const work = await rpcClient.getMiningWork();
      const json = JSON.stringify(work.problem, null, 2);
      setFiles((prev) => ({ ...prev, "instance.json": json }));
      setActiveFile("instance.json");
      setRunResult(null);
      setConsoleLines((prev) => [
        ...prev,
        `[chain] Live mining instance for block #${work.next_height}`,
        `[chain] prev_hash (epoch salt): ${work.prev_hash.slice(0, 24)}…`,
        "[chain] Run solver, then Submit — if the tip advances, sync from chain again before submitting.",
        "",
      ]);
      await queryClient.invalidateQueries({ queryKey: ["solverLab"] });
      toast.success("instance.json loaded from chain");
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setConsoleLines((prev) => [...prev, `[error] chain_getMiningWork: ${msg}`, ""]);
      toast.error("Could not load live instance", { description: msg });
    } finally {
      setPullingChainInstance(false);
    }
  };

  /** Replace `instance.json` with the canonical default for `kind`. Optionally keep another file focused (e.g. open `sat.js` while loading a SAT instance). */
  const loadInstanceTemplate = (kind: NetworkProblemKind, focusFile: WorkspaceFilePath = "instance.json") => {
    setFiles((prev) => ({ ...prev, "instance.json": DEFAULT_PROBLEM_JSON[kind] }));
    setActiveFile(focusFile);
  };

  /** When opening a solver tab, load a matching network instance if the current `instance.json` is for another problem type. */
  const onSelectWorkspaceFile = (path: WorkspaceFilePath) => {
    if (path === "solvers/sat.js" && instanceProblemKind !== "SAT") {
      loadInstanceTemplate("SAT", "solvers/sat.js");
      return;
    }
    if (path === "solvers/tsp.js" && instanceProblemKind !== "TSP") {
      loadInstanceTemplate("TSP", "solvers/tsp.js");
      return;
    }
    if (path === "solvers/subset-sum.js" && instanceProblemKind !== "SubsetSum") {
      loadInstanceTemplate("SubsetSum", "solvers/subset-sum.js");
      return;
    }
    setActiveFile(path);
  };

  const submitToBounty = () => {
    const parsed = parseNetworkProblem(instanceText);
    if (!parsed.ok) {
      setConsoleLines((prev) => [...prev, `[error] Fix instance.json before submitting: ${parsed.error}`]);
      return;
    }
    const kind: NetworkProblemKind = parsed.value.SubsetSum
      ? "SubsetSum"
      : parsed.value.SAT
        ? "SAT"
        : "TSP";
    const title = `Solver Lab — ${NETWORK_REGISTRY[kind].label}`;
    const description = [
      "## Network instance (`instance.json`)",
      "```json",
      instanceText.trim(),
      "```",
      "",
      "## Your solvers (owned code)",
      ...WORKSPACE_FILES.filter((f) => f !== "instance.json").map((f) => {
        return [`### ${f}`, "```javascript", files[f].trim(), "```"].join("\n");
      }),
      "",
      runResult?.solution
        ? "## Last run solution (reference)\n\n```json\n" + JSON.stringify(runResult.solution, null, 2) + "\n```"
        : "",
      "",
      "_Draft from Solver Lab (Remix-style workspace). Algorithms above are yours to license._",
    ].join("\n");

    try {
      sessionStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({
          problemType: kind,
          title,
          description,
          draftKind: "solver" as const,
        })
      );
    } catch {
      setConsoleLines((prev) => [...prev, "[error] Could not store draft"]);
      return;
    }
    navigate("/bounty-submit");
  };

  /**
   * Run your solver on `instance.json`, build a block with commitment to the current tip hash (epoch salt),
   * mine the header, and submit via RPC. On success, open the Wallet page.
   */
  const submitProblemToChain = async () => {
    if (!selectedKeyPair) {
      toast.error("Wallet required", { description: "Create or select an account on the Wallet page first." });
      navigate("/wallet");
      return;
    }

    const parsed = parseNetworkProblem(instanceText);
    if (!parsed.ok) {
      setConsoleLines((prev) => [...prev, `[error] Fix instance.json before submitting: ${parsed.error}`]);
      return;
    }

    setSubmittingChain(true);
    try {
      const work = await rpcClient.getMiningWork();
      if (!problemTypesEqual(parsed.value, work.problem)) {
        const msg =
          "instance.json must match chain_getMiningWork (next block template). Click “Sync from chain”, then solve and submit.";
        setConsoleLines((prev) => [...prev, `[error] ${msg}`, ""]);
        toast.error("Instance out of sync with chain", { description: "Use “Sync from chain” at the top, then submit again." });
        return;
      }

      const prevHashHex = extractHashHex(work.prev_hash);
      const nextHeight = work.next_height;

      const out = await runUserSolver(files, parsed.value, 45000);
      if (!out.ok) {
        setConsoleLines((prev) => [...prev, `[error] Solver: ${out.error}`, ""]);
        return;
      }
      const normalized = normalizeSolution(parsed.value, out.solution);
      if (!normalized) {
        setConsoleLines((prev) => [...prev, "[error] Solution shape invalid — cannot commit to chain.", ""]);
        return;
      }

      const miningSolution: MiningSolution = {
        SubsetSum: normalized.SubsetSum,
        SAT: normalized.SAT,
        TSP: normalized.TSP,
        Custom: normalized.Custom,
      };

      setConsoleLines((prev) => [
        ...prev,
        `[chain] Mining template block #${nextHeight} prev_hash: ${prevHashHex.slice(0, 16)}…`,
        `[chain] Building block + PoW; coinbase → your wallet address…`,
        "",
      ]);

      const block = await createBlockFromSolvedProblem(
        prevHashHex,
        nextHeight,
        selectedKeyPair.address,
        parsed.value,
        miningSolution,
        out.timeMs,
        [],
        2
      );
      if (!block) {
        setConsoleLines((prev) => [...prev, "[error] Solution failed verification or PoW header mining failed.", ""]);
        return;
      }

      const finalChainInfo = await rpcClient.getChainInfo();
      if (finalChainInfo.best_height >= block.header.height) {
        setConsoleLines((prev) => [...prev, "[error] Chain advanced during mining. Sync from chain again and retry.", ""]);
        return;
      }
      if (finalChainInfo.best_hash && extractHashHex(finalChainInfo.best_hash) !== prevHashHex) {
        setConsoleLines((prev) => [...prev, "[error] Tip hash changed. Sync from chain again and retry.", ""]);
        return;
      }

      const blockHash = await rpcClient.submitBlock(block);
      setConsoleLines((prev) => [
        ...prev,
        `[chain] Submitted block #${block.header.height}`,
        `[chain] Block hash: ${blockHash.slice(0, 16)}…`,
        `[chain] Miner reward credited to ${selectedKeyPair.address.slice(0, 12)}… (refresh Wallet)`,
        "",
      ]);
      await queryClient.invalidateQueries({ queryKey: ["accountInfo", selectedKeyPair.address] });
      await queryClient.invalidateQueries({ queryKey: ["balance", selectedKeyPair.address] });
      await queryClient.invalidateQueries({ queryKey: ["solverLab"] });
      toast.success("Block submitted — miner reward sent to your wallet", { description: "Opening Wallet…" });
      navigate("/wallet");
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setConsoleLines((prev) => [...prev, `[error] ${msg}`, ""]);
      toast.error("Chain submit failed", { description: msg });
    } finally {
      setSubmittingChain(false);
    }
  };

  const renderViz = () => {
    if (!parsedPreview.ok) {
      return <p className="text-sm text-destructive">{parsedPreview.error}</p>;
    }
    const p = parsedPreview.value;
    const sol = runResult?.ok ? runResult.solution : null;

    if (p.SubsetSum) {
      const idx = sol?.SubsetSum ?? null;
      return <SubsetSumVisualizer numbers={p.SubsetSum.numbers} selected={idx} />;
    }
    if (p.SAT) {
      const a = sol?.SAT ?? null;
      return (
        <SATVisualizer
          variables={p.SAT.variables}
          clauses={p.SAT.clauses}
          assignment={a && a.length === p.SAT.variables ? a : null}
        />
      );
    }
    if (p.TSP) {
      const tour = sol?.TSP ?? null;
      return <TSPVisualizer cities={p.TSP.cities} distances={p.TSP.distances} tour={tour} />;
    }
    return null;
  };

  const isDark = resolvedTheme === "dark";

  const instanceMismatchAlert =
    instanceKindMismatch && activeSolverExpectedKind ? (
      <Alert className="mb-3 border-amber-500/40 bg-amber-500/5">
        <AlertTitle className="text-sm">instance.json does not match this solver tab</AlertTitle>
        <AlertDescription className="flex flex-col gap-2 text-xs">
          <span>
            Visualization and Run use <code className="text-[11px]">instance.json</code> ({instanceProblemKind}). Open a
            matching instance or load the default for {activeSolverExpectedKind}.
          </span>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            className="w-fit h-8"
            onClick={() => loadInstanceTemplate(activeSolverExpectedKind, activeFile)}
          >
            Load {activeSolverExpectedKind} instance
          </Button>
        </AlertDescription>
      </Alert>
    ) : null;

  return (
    <div className={cn("flex flex-col gap-3 min-h-0 w-full min-w-0 flex-1", className)}>
      <Tabs value={workspace} onValueChange={(v) => setWorkspace(v as "solvers" | "chain")} className="flex flex-col flex-1 min-h-0 gap-3 w-full min-w-0">
        <TabsList className="w-full grid grid-cols-2 h-10 sm:h-11">
          <TabsTrigger value="solvers">Solver Lab</TabsTrigger>
          <TabsTrigger value="chain" className="gap-2">
            <Network className="h-4 w-4" />
            Chain CLI
          </TabsTrigger>
        </TabsList>

        <TabsContent value="chain" className="flex-1 min-h-0 mt-0 data-[state=inactive]:hidden w-full min-w-0 flex flex-col">
          <WebCliTerminal compact className="w-full max-w-none min-h-[min(42dvh,380px)] lg:min-h-0 flex-1" />
        </TabsContent>

        <TabsContent value="solvers" className="flex-1 flex flex-col min-h-0 mt-0 data-[state=inactive]:hidden w-full min-w-0">
          <div className="mb-2 rounded-lg border border-border/50 bg-muted/25 px-3 py-2 text-[11px] sm:text-xs text-muted-foreground flex flex-wrap items-center gap-x-3 gap-y-1 shrink-0">
            <span className="font-medium text-foreground/90">Chain</span>
            {chainInfo ? (
              <>
                <span>
                  Tip <span className="font-mono text-foreground">{chainInfo.best_height}</span>
                </span>
                <span>·</span>
                <span>{chainInfo.peer_count} peers</span>
                {chainInfo.is_syncing ? (
                  <>
                    <span>·</span>
                    <span className="text-amber-600 dark:text-amber-400">syncing</span>
                  </>
                ) : null}
              </>
            ) : (
              <span>loading…</span>
            )}
            {miningWork ? (
              <>
                <span>·</span>
                <span>
                  Next block <span className="font-mono text-foreground">{miningWork.next_height}</span>
                </span>
              </>
            ) : null}
            <span>·</span>
            {parsedPreview.ok ? (
              instanceMatchesMiningWork ? (
                <span className="text-emerald-600 dark:text-emerald-400">instance.json matches mining work</span>
              ) : (
                <span className="inline-flex flex-wrap items-center gap-2">
                  <span className="text-amber-600 dark:text-amber-400">
                    instance.json must match the next block — sync before Submit
                  </span>
                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    className="h-7 text-xs px-2 shrink-0"
                    onClick={() => void pullLiveInstanceFromChain()}
                    disabled={pullingChainInstance || running}
                    title="Load chain_getMiningWork into instance.json (same template as miners)"
                  >
                    {pullingChainInstance ? "Loading…" : "Sync from chain"}
                  </Button>
                </span>
              )
            ) : (
              <span className="text-destructive">fix instance.json JSON</span>
            )}
          </div>
          {isLg ? (
          <div className="flex h-[min(72dvh,720px)] w-full min-w-0 flex-1 flex-col overflow-hidden rounded-lg border border-border/60 bg-background/50 min-h-[min(68dvh,560px)] lg:h-[min(86dvh,calc(100dvh-13rem))] lg:min-h-[520px] lg:flex-row">
            {/* File explorer — full height; console does not span under this column */}
            <aside className="flex shrink-0 flex-col border-b border-border/60 bg-muted/15 lg:h-full lg:w-52 lg:self-stretch lg:border-b-0 lg:border-r">
              <div className="flex items-center gap-2 px-2 py-2 border-b border-border/50 text-xs font-medium text-muted-foreground">
                <FolderOpen className="h-3.5 w-3.5" />
                Workspace
              </div>
              <ScrollArea className="flex-1 min-h-0 lg:flex-1 lg:min-h-0 max-h-[200px] lg:max-h-none">
                <div className="p-1 space-y-0.5">
                  {WORKSPACE_FILES.map((path) => {
                    const meta = FILE_META[path];
                    const Icon = meta.icon;
                    const active = activeFile === path;
                    return (
                      <button
                        key={path}
                        type="button"
                        onClick={() => onSelectWorkspaceFile(path)}
                        className={cn(
                          "w-full flex items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs font-mono transition-colors",
                          active ? "bg-primary/15 text-foreground" : "hover:bg-muted/60 text-muted-foreground"
                        )}
                      >
                        <Icon className="h-3.5 w-3.5 shrink-0 opacity-80" />
                        <span className="truncate">{meta.label}</span>
                      </button>
                    );
                  })}
                </div>
              </ScrollArea>
              <div className="p-2 border-t border-border/50 space-y-1">
                <p className="text-[10px] text-muted-foreground px-1">Load instance from chain</p>
                <div className="flex flex-wrap gap-1">
                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    className="h-7 text-[10px] px-2"
                    onClick={() => void pullLiveInstanceFromChain()}
                    disabled={pullingChainInstance || running}
                    title="Fetch chain_getMiningWork (same instance as miners for the next block)"
                  >
                    {pullingChainInstance ? "…" : "Sync"}
                  </Button>
                  {(["SubsetSum", "SAT", "TSP"] as const).map((k) => (
                    <Button
                      key={k}
                      type="button"
                      variant="outline"
                      size="sm"
                      className="h-7 text-[10px] px-2"
                      onClick={() => loadInstanceTemplate(k)}
                    >
                      {k === "SubsetSum" ? "SS" : k}
                    </Button>
                  ))}
                </div>
              </div>
            </aside>

            <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden lg:h-full lg:min-h-0">
            <ResizablePanelGroup
              direction="vertical"
              className="h-full min-h-[360px] w-full flex-1"
            >
              <ResizablePanel defaultSize={72} minSize={38} className="min-h-0">
                <ResizablePanelGroup
                  direction="horizontal"
                  className="flex h-full min-h-[220px] w-full min-w-0"
                >
                  <ResizablePanel defaultSize={52} minSize={30} className="min-w-0 flex flex-col min-h-0">
                    <div className="flex items-center justify-between gap-2 px-3 py-2 border-b border-border/50 bg-muted/20 text-xs shrink-0">
                      <span className="font-mono text-muted-foreground truncate">{activeFile}</span>
                      <div className="flex items-center gap-1 shrink-0">
                        <Button type="button" variant="ghost" size="sm" className="h-8 gap-1" onClick={resetAll} title="Reset">
                          <RotateCcw className="h-3.5 w-3.5" />
                        </Button>
                        <Button
                          type="button"
                          size="sm"
                          className="h-8 gap-1 max-w-[min(100%,14rem)]"
                          onClick={run}
                          disabled={running || submittingChain || pullingChainInstance}
                          title={runButtonTitle}
                        >
                          <Play className="h-3.5 w-3.5 shrink-0" />
                          <span className="truncate">{runButtonLabel}</span>
                        </Button>
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          className="h-8 gap-1 max-w-[min(100%,11rem)]"
                          onClick={() => void submitProblemToChain()}
                          disabled={submittingChain || running || pullingChainInstance}
                          title="Run solver, commit with tip hash as epoch salt, mine PoW, submitBlock. Requires wallet."
                        >
                          <Link2 className="h-3.5 w-3.5 shrink-0" />
                          <span className="truncate">{submittingChain ? "…" : "Submit problem"}</span>
                        </Button>
                        <Button type="button" variant="outline" size="sm" className="h-8 gap-1" onClick={submitToBounty}>
                          <Send className="h-3.5 w-3.5" />
                          Bounty
                        </Button>
                      </div>
                    </div>
                    <div className="flex-1 min-h-0 min-w-0 p-2 flex flex-col">
                      <Editor
                        key={activeFile}
                        path={activeFile}
                        value={files[activeFile]}
                        onChange={(v) => onEditorChange(v)}
                        dark={isDark}
                        minHeight="100%"
                        className="min-h-[200px] flex-1 h-full"
                      />
                    </div>
                  </ResizablePanel>
                  <ResizableHandle withHandle />
                  <ResizablePanel defaultSize={48} minSize={28} className="min-w-0 min-h-0">
                    <Card className="h-full border-0 rounded-none shadow-none flex flex-col min-h-0">
                      <CardHeader className="py-3 px-4 border-b border-border/50">
                        <CardTitle className="text-sm font-medium">Visualization & results</CardTitle>
                      </CardHeader>
                      <CardContent className="flex-1 p-4 overflow-auto min-h-0">
                        {instanceMismatchAlert}
                        <Tabs defaultValue="viz" className="w-full">
                          <TabsList className="mb-3">
                            <TabsTrigger value="viz">Visual</TabsTrigger>
                            <TabsTrigger value="json">Result</TabsTrigger>
                          </TabsList>
                          <TabsContent value="viz" className="mt-0">
                            {renderViz()}
                          </TabsContent>
                          <TabsContent value="json" className="mt-0">
                            <pre className="text-xs font-mono whitespace-pre-wrap break-words bg-muted/30 rounded-md p-3 border border-border/50">
                              {runResult
                                ? JSON.stringify(
                                    {
                                      ok: runResult.ok,
                                      timeMs: runResult.timeMs,
                                      solution: runResult.solution,
                                    },
                                    null,
                                    2
                                  )
                                : "Run the solver to see structured output."}
                            </pre>
                          </TabsContent>
                        </Tabs>
                      </CardContent>
                    </Card>
                  </ResizablePanel>
                </ResizablePanelGroup>
              </ResizablePanel>

              <ResizableHandle withHandle />

              <ResizablePanel defaultSize={28} minSize={16} className="min-h-[88px]">
                <Card className="flex h-full min-h-0 flex-col border-0 border-t border-border/60 shadow-none lg:border-t-0">
                  <CardHeader className="py-2 px-3 flex flex-row items-center justify-between gap-2 shrink-0">
                    <CardTitle className="text-sm font-medium">Console</CardTitle>
                    <Button type="button" variant="ghost" size="sm" className="h-8 gap-1" onClick={clearConsole}>
                      <Trash2 className="h-3.5 w-3.5" />
                      Clear
                    </Button>
                  </CardHeader>
                  <CardContent className="flex-1 p-0 min-h-0 flex flex-col overflow-hidden">
                    <ScrollArea className="flex-1 min-h-[120px] h-full">
                      <pre className="text-xs font-mono p-3 pr-6 whitespace-pre-wrap break-words text-muted-foreground">
                        {consoleLines.length === 0 ? "Build output and errors appear here." : consoleLines.join("\n")}
                      </pre>
                    </ScrollArea>
                  </CardContent>
                </Card>
              </ResizablePanel>
            </ResizablePanelGroup>
            </div>
          </div>
          ) : (
            <div className="flex flex-col flex-1 min-h-0 min-h-[min(70dvh,560px)] w-full rounded-lg border border-border/60 bg-background/50 overflow-hidden pb-[max(0.75rem,env(safe-area-inset-bottom))] touch-manipulation">
              <div className="shrink-0 z-10 border-b border-border/60 bg-background/95 backdrop-blur-md px-3 pt-2 pb-2 space-y-2">
                <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
                  <Select
                    value={activeFile}
                    onValueChange={(v) => onSelectWorkspaceFile(v as WorkspaceFilePath)}
                  >
                    <SelectTrigger className="h-11 min-h-[44px] w-full sm:max-w-xs font-mono text-xs">
                      <SelectValue placeholder="Workspace file" />
                    </SelectTrigger>
                    <SelectContent>
                      {WORKSPACE_FILES.map((path) => (
                        <SelectItem key={path} value={path} className="font-mono text-xs">
                          {FILE_META[path].label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="text-[10px] uppercase tracking-wide text-muted-foreground">Instance</span>
                    <div className="flex gap-1.5 flex-wrap">
                      <Button
                        type="button"
                        variant="secondary"
                        size="sm"
                        className="h-11 min-h-[44px] min-w-[3rem] px-3 text-xs"
                        onClick={() => void pullLiveInstanceFromChain()}
                        disabled={pullingChainInstance || running}
                        title="Load instance.json from chain_getMiningWork"
                      >
                        {pullingChainInstance ? "…" : "Sync"}
                      </Button>
                      {(["SubsetSum", "SAT", "TSP"] as const).map((k) => (
                        <Button
                          key={k}
                          type="button"
                          variant="outline"
                          size="sm"
                          className="h-11 min-h-[44px] min-w-[2.75rem] px-3 text-xs"
                          onClick={() => loadInstanceTemplate(k)}
                        >
                          {k === "SubsetSum" ? "SS" : k}
                        </Button>
                      ))}
                    </div>
                  </div>
                </div>
                <div className="flex flex-wrap gap-2">
                  <Button
                    type="button"
                    size="default"
                    className="h-11 min-h-[44px] flex-1 min-w-[8rem] sm:flex-none"
                    onClick={run}
                    disabled={running || submittingChain || pullingChainInstance}
                    title={runButtonTitle}
                  >
                    <Play className="h-4 w-4 mr-2 shrink-0" />
                    <span className="truncate">{runButtonLabel}</span>
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="default"
                    className="h-11 min-h-[44px] flex-1 min-w-[8rem] sm:flex-none"
                    onClick={() => void submitProblemToChain()}
                    disabled={submittingChain || running || pullingChainInstance}
                    title="Submit solution to chain (epoch salt = tip hash)"
                  >
                    <Link2 className="h-4 w-4 mr-2 shrink-0" />
                    <span className="truncate">{submittingChain ? "…" : "Submit"}</span>
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="default"
                    className="h-11 min-h-[44px] flex-1 min-w-[8rem] sm:flex-none"
                    onClick={submitToBounty}
                  >
                    <Send className="h-4 w-4 mr-2 shrink-0" />
                    Bounty
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="h-11 min-h-[44px] w-11 shrink-0"
                    onClick={resetAll}
                    title="Reset workspace"
                  >
                    <RotateCcw className="h-5 w-5" />
                  </Button>
                </div>
              </div>

              <Tabs
                value={mobilePanel}
                onValueChange={(v) => setMobilePanel(v as "code" | "visual" | "result" | "console")}
                className="flex flex-col flex-1 min-h-0"
              >
                <TabsList className="grid w-full grid-cols-4 h-12 shrink-0 rounded-none border-b border-border/50 bg-muted/20 p-1">
                  <TabsTrigger value="code" className="text-xs px-1">
                    Code
                  </TabsTrigger>
                  <TabsTrigger value="visual" className="text-xs px-1">
                    Visual
                  </TabsTrigger>
                  <TabsTrigger value="result" className="text-xs px-1">
                    Result
                  </TabsTrigger>
                  <TabsTrigger value="console" className="text-xs px-1 gap-1">
                    <Terminal className="h-3.5 w-3.5 opacity-80" aria-hidden />
                    Log
                  </TabsTrigger>
                </TabsList>
                <TabsContent value="code" className="flex-1 min-h-0 mt-0 overflow-hidden p-2 data-[state=inactive]:hidden">
                  <Editor
                    key={activeFile}
                    path={activeFile}
                    value={files[activeFile]}
                    onChange={(v) => onEditorChange(v)}
                    dark={isDark}
                    minHeight="min(52dvh, 420px)"
                    className="min-h-[240px]"
                  />
                </TabsContent>
                <TabsContent value="visual" className="flex-1 min-h-0 mt-0 overflow-y-auto p-3 data-[state=inactive]:hidden">
                  {instanceMismatchAlert}
                  {renderViz()}
                </TabsContent>
                <TabsContent value="result" className="flex-1 min-h-0 mt-0 overflow-y-auto p-3 data-[state=inactive]:hidden">
                  <pre className="text-xs font-mono whitespace-pre-wrap break-words bg-muted/30 rounded-md p-3 border border-border/50">
                    {runResult
                      ? JSON.stringify(
                          {
                            ok: runResult.ok,
                            timeMs: runResult.timeMs,
                            solution: runResult.solution,
                          },
                          null,
                          2
                        )
                      : "Run the solver to see structured output."}
                  </pre>
                </TabsContent>
                <TabsContent value="console" className="flex-1 min-h-0 mt-0 overflow-hidden p-0 data-[state=inactive]:hidden flex flex-col">
                  <div className="flex items-center justify-between gap-2 px-3 py-2 border-b border-border/50 bg-muted/10">
                    <span className="text-xs font-medium text-muted-foreground">Console</span>
                    <Button type="button" variant="ghost" size="sm" className="h-9 min-h-[44px]" onClick={clearConsole}>
                      <Trash2 className="h-4 w-4 mr-1" />
                      Clear
                    </Button>
                  </div>
                  <ScrollArea className="flex-1 min-h-[min(40dvh,280px)]">
                    <pre className="text-xs font-mono p-3 pr-6 whitespace-pre-wrap break-words text-muted-foreground">
                      {consoleLines.length === 0 ? "Build output and errors appear here." : consoleLines.join("\n")}
                    </pre>
                  </ScrollArea>
                </TabsContent>
              </Tabs>
            </div>
          )}
        </TabsContent>
      </Tabs>
    </div>
  );
}
