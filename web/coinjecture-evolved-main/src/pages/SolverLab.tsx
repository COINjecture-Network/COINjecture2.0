import { useEffect } from "react";
import { Navigation } from "@/components/Navigation";
import { NpPlayground } from "@/features/np-playground/NpPlayground";

const SolverLab = () => {
  useEffect(() => {
    document.title = "Solver Lab — COINjecture";
  }, []);

  return (
    <div className="min-h-[100dvh] bg-background flex flex-col lg:h-[100dvh] lg:overflow-hidden">
      <Navigation />
      <main className="flex-1 pt-20 sm:pt-24 lg:pt-28 flex flex-col min-h-0 lg:overflow-hidden pb-[max(0.5rem,env(safe-area-inset-bottom))]">
        <div className="container mx-auto px-3 sm:px-6 shrink-0">
          <header className="mb-2 sm:mb-4">
            <div className="market-surface-strong p-3 sm:p-4 md:p-5">
              <div className="max-w-5xl mx-auto">
                <div className="signal-kicker text-center hidden sm:block">Miner workbench</div>
                <h1 className="text-xl sm:text-3xl font-bold mb-1 sm:mb-2 tracking-tight text-center">
                  Solver <span className="text-primary">Lab</span>
                </h1>
                <p className="text-muted-foreground text-center max-w-3xl mx-auto text-sm leading-relaxed hidden md:block">
                  Write your own solver, sync the next live chain instance, test locally, then submit a block when your wallet is ready.
                </p>
                <div className="hidden md:grid gap-2 md:grid-cols-4 mt-3 sm:mt-4">
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
        <div className="flex-1 min-h-[min(72dvh,640px)] lg:min-h-0 flex flex-col w-full min-w-0 px-2 sm:px-4 lg:px-6 lg:overflow-hidden">
          <NpPlayground className="flex-1 min-h-0 flex flex-col h-full" />
        </div>
      </main>
    </div>
  );
};

export default SolverLab;
