import { useEffect } from "react";
import { Navigation } from "@/components/Navigation";
import { Footer } from "@/components/Footer";
import { NpPlayground } from "@/features/np-playground/NpPlayground";

const SolverLab = () => {
  useEffect(() => {
    document.title = "Solver Lab — COINjecture";
  }, []);

  return (
    <div className="min-h-screen bg-background flex flex-col">
      <Navigation />
      <main className="flex-1 pt-32 flex flex-col min-h-0 pb-[max(5rem,env(safe-area-inset-bottom))]">
        <div className="container mx-auto px-6 shrink-0">
          <header className="text-center mb-12">
            <h1 className="text-4xl font-bold mb-4 tracking-tight">
              Solver <span className="text-primary">Lab</span>
            </h1>
            <div className="text-muted-foreground leading-relaxed space-y-3 text-left max-w-4xl mx-auto">
              <p>
                <strong className="text-foreground font-medium">Your code, your rules.</strong> The algorithms you write in this workspace
                are yours. They stay in this browser only—nothing is sent to the network until you use{" "}
                <strong className="text-foreground">Submit problem</strong>, <strong className="text-foreground">Bounty</strong>, or{" "}
                <strong className="text-foreground">Chain CLI</strong>.
              </p>
              <p>
                <strong className="text-foreground font-medium">What to edit:</strong>{" "}
                <code className="text-xs text-foreground">solvers/*.js</code> holds your solver functions;{" "}
                <code className="text-xs text-foreground">instance.json</code> defines the network{" "}
                <code className="text-xs text-foreground">ProblemType</code> (same JSON shape and verification rules as{" "}
                <code className="text-xs text-foreground">mining.ts</code> and the RPC). Use{" "}
                <strong className="text-foreground">Sync from chain</strong> (or <strong className="text-foreground">Sync</strong> in the sidebar) to pull{" "}
                <code className="text-xs text-foreground">chain_getMiningWork</code> from the RPC—the same deterministic instance
                validating miners use for the next block.
              </p>
              <p>
                <strong className="text-foreground font-medium">What each action does:</strong>{" "}
                <strong className="text-foreground">Run</strong> executes your workspace in a web worker (with a timeout) so you can test
                locally. <strong className="text-foreground">Submit problem</strong> runs your solver, then builds and submits a block to
                the network using the current tip hash as the epoch salt (requires a wallet).{" "}
                <strong className="text-foreground">Bounty</strong> prepares a draft of your problem and source files for bounty
                submission. <strong className="text-foreground">Chain CLI</strong> is for wallet and node commands against the RPC.
              </p>
            </div>
          </header>
        </div>
        <div className="flex-1 min-h-0 flex flex-col w-full min-w-0 px-3 sm:px-4 lg:px-6">
          <NpPlayground className="flex-1 min-h-0 flex flex-col" />
        </div>
      </main>
      <Footer />
    </div>
  );
};

export default SolverLab;
