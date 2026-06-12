import { useEffect } from "react";
import { Link } from "react-router-dom";
import { Navigation } from "@/components/Navigation";
import { NpPlayground } from "@/features/np-playground/NpPlayground";

const SolverLab = () => {
  useEffect(() => {
    document.title = "Solver Lab — COINjecture";
  }, []);

  return (
    <div className="min-h-[100dvh] bg-background">
      <Navigation />
      {/* Desktop: pin workbench to viewport below fixed nav (reliable IDE height). Mobile: stacked + scroll. */}
      <main className="flex h-[100dvh] flex-col overflow-hidden pt-20 sm:pt-24 lg:fixed lg:inset-x-0 lg:bottom-0 lg:top-14 lg:z-0 lg:h-[calc(100dvh-3.5rem)] lg:pt-0">
        {/* Marketing header — tablet/mobile only */}
        <div className="container mx-auto shrink-0 px-3 sm:px-6 lg:hidden">
          <header className="mb-2 sm:mb-3">
            <div className="market-surface-strong p-3 sm:p-4">
              <div className="max-w-5xl mx-auto">
                <div className="signal-kicker text-center hidden sm:block">Miner workbench</div>
                <h1 className="text-xl sm:text-3xl font-bold mb-1 sm:mb-2 tracking-tight text-center">
                  Solver <span className="text-primary">Lab</span>
                </h1>
                <p className="text-muted-foreground text-center max-w-3xl mx-auto text-sm leading-relaxed hidden md:block">
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
        <div className="flex h-0 min-w-0 flex-1 flex-col px-2 pb-[max(0.5rem,env(safe-area-inset-bottom))] sm:px-4 lg:h-full lg:min-h-0 lg:overflow-hidden lg:px-3 lg:pb-2">
          <NpPlayground className="flex h-full min-h-0 w-full flex-1 flex-col" />
        </div>
      </main>
    </div>
  );
};

export default SolverLab;
