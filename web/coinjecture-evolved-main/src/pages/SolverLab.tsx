import { useEffect } from "react";
import { Link } from "react-router-dom";
import { Navigation } from "@/components/Navigation";
import { NpPlayground } from "@/features/np-playground/NpPlayground";

const SolverLab = () => {
  useEffect(() => {
    document.title = "Solver Lab — COINjecture";
  }, []);

  return (
    <div className="min-h-[100dvh] bg-background flex flex-col lg:h-[100dvh] lg:max-h-[100dvh] lg:overflow-hidden">
      <Navigation />
      <main className="flex flex-1 flex-col min-h-0 lg:h-full lg:overflow-hidden pt-20 sm:pt-24 lg:pt-24 pb-[max(0.5rem,env(safe-area-inset-bottom))]">
        <div className="container mx-auto shrink-0 px-3 sm:px-6 lg:px-4">
          <header className="mb-2 sm:mb-3 lg:mb-2">
            <div className="market-surface-strong p-3 sm:p-4 lg:p-3">
              <div className="max-w-5xl mx-auto">
                <div className="signal-kicker text-center hidden sm:block lg:hidden">Miner workbench</div>
                <h1 className="text-xl sm:text-3xl lg:text-2xl font-bold mb-1 sm:mb-2 lg:mb-0 tracking-tight text-center">
                  Solver <span className="text-primary">Lab</span>
                </h1>
                <p className="text-muted-foreground text-center max-w-3xl mx-auto text-sm leading-relaxed hidden md:block lg:hidden">
                  Write your own solver, sync the next live chain instance, test locally, then submit a block when your wallet is ready.
                </p>
                <div className="hidden md:flex justify-center gap-2 mt-3 flex-wrap">
                  <Link
                    to="/marketplace"
                    className="text-xs text-primary underline-offset-4 hover:underline"
                  >
                    Browse open bounties
                  </Link>
                  <span className="text-xs text-muted-foreground">·</span>
                  <Link
                    to="/bounty-submit"
                    className="text-xs text-primary underline-offset-4 hover:underline"
                  >
                    Post a bounty
                  </Link>
                </div>
                <div className="hidden md:grid lg:hidden gap-2 md:grid-cols-4 mt-3 sm:mt-4">
                  <div className="signal-card">
                    <div className="signal-kicker">1. Prepare</div>
                    <div className="mt-2 font-semibold">Create or connect a wallet.</div>
                  </div>
                  <div className="signal-card">
                    <div className="signal-kicker">2. Sync</div>
                    <div className="mt-2 font-semibold">Pull `chain_getMiningWork` into `instance.json`.</div>
                  </div>
                  <div className="signal-card">
                    <div className="signal-kicker">3. Run</div>
                    <div className="mt-2 font-semibold">Test your solver locally in the browser worker.</div>
                  </div>
                  <div className="signal-card">
                    <div className="signal-kicker">4. Submit</div>
                    <div className="mt-2 font-semibold">Mine and broadcast a block or draft a bounty.</div>
                  </div>
                </div>
              </div>
            </div>
          </header>
        </div>
        <div className="flex min-h-[min(72dvh,640px)] flex-1 flex-col w-full min-w-0 px-2 sm:px-4 lg:min-h-0 lg:flex-1 lg:overflow-hidden lg:px-4">
          <NpPlayground className="flex h-full min-h-0 flex-1 flex-col lg:min-h-[calc(100dvh-7.5rem)]" />
        </div>
      </main>
    </div>
  );
};

export default SolverLab;
