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
          <header className="mb-12">
            <div className="market-surface-strong p-6 md:p-8">
              <div className="max-w-5xl mx-auto">
                <div className="signal-kicker text-center">Miner workbench</div>
                <h1 className="text-4xl font-bold mb-4 tracking-tight text-center">
                  Solver <span className="text-primary">Lab</span>
                </h1>
                <p className="text-muted-foreground text-center max-w-3xl mx-auto leading-relaxed">
                  Write your own solver, sync the next live chain instance, test locally, then submit a block when your wallet is ready.
                </p>
                <div className="grid gap-3 md:grid-cols-4 mt-8">
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
        <div className="flex-1 min-h-0 flex flex-col w-full min-w-0 px-3 sm:px-4 lg:px-6">
          <NpPlayground className="flex-1 min-h-0 flex flex-col" />
        </div>
      </main>
      <Footer />
    </div>
  );
};

export default SolverLab;
