import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { BrandLogo } from "@/components/BrandLogo";
import { ArrowRight, Download, Code, Award, Target, TrendingUp, Database, Loader2, Server, Coins, CheckCircle2, Calculator } from "lucide-react";
import { Link } from "react-router-dom";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { rpcClient } from "@/lib/rpc-client";
import { formatBeans, parseBalance } from "@/lib/chain-metrics";
import { hfDatasetPageUrl } from "@/lib/hf-dataset";

const useHeroVideo = () => {
  const [showVideo, setShowVideo] = useState(
    () => typeof window !== "undefined" && window.matchMedia("(min-width: 768px)").matches
  );

  useEffect(() => {
    const mq = window.matchMedia("(min-width: 768px)");
    const apply = () => setShowVideo(mq.matches);
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, []);

  return showVideo;
};

export const Hero = () => {
  const showHeroVideo = useHeroVideo();

  const { data: chainInfo } = useQuery({
    queryKey: ['chain-info'],
    queryFn: () => rpcClient.getChainInfo(),
    refetchInterval: 10000,
  });

  const { data: marketplaceStats } = useQuery({
    queryKey: ['marketplace-stats'],
    queryFn: () => rpcClient.getMarketplaceStats(),
    refetchInterval: 30000,
  });

  return (
    <>
      {/* Hero Section */}
      <section className="min-h-[100dvh] min-h-screen pt-32 pb-20 relative overflow-hidden flex flex-col">
        <div
          className={
            showHeroVideo
              ? "absolute inset-0 z-0 hero-video-fallback"
              : "absolute inset-0 z-0 hero-video-fallback-mobile"
          }
          aria-hidden
        />
        {showHeroVideo && (
          <video
            className="hero-bg-video absolute inset-0 z-[1] h-full w-full object-cover object-top opacity-50"
            src="/Improving_Educational_Video_Content.mp4"
            autoPlay
            muted
            loop
            playsInline
            aria-hidden
          />
        )}
        <div
          className="absolute inset-0 z-[2] bg-gradient-to-b from-background/88 via-background/72 to-background/92 dark:from-background/92 dark:via-background/75 dark:to-background/96"
          aria-hidden
        />
        <div
          className="absolute inset-0 z-[2] bg-gradient-to-br from-primary/25 via-accent-purple/10 to-accent-emerald/20 pointer-events-none"
          aria-hidden
        />

        <div
          className="absolute top-28 left-[8%] w-3 h-3 rounded-full bg-accent-blue animate-float-gentle opacity-60 pointer-events-none z-[3] hidden md:block"
          aria-hidden
        />
        <div
          className="absolute top-40 right-[12%] w-4 h-4 rounded-full bg-accent-emerald animate-drift-left opacity-45 pointer-events-none z-[3] hidden md:block"
          aria-hidden
        />
        <div
          className="absolute bottom-40 left-1/4 w-3.5 h-3.5 rounded-full bg-accent-purple animate-drift-right opacity-50 pointer-events-none z-[3] hidden md:block"
          aria-hidden
        />

        <div className="container mx-auto px-6 relative z-10 flex-1 flex flex-col justify-center">
          <div className="max-w-6xl mx-auto w-full">
            <div className="text-center mb-16 animate-fade-in">
              <div className="flex justify-center mb-6">
                <BrandLogo size="lg" />
              </div>
              <div className="inline-flex items-center gap-2 px-4 py-2 rounded-full glass-effect border-white/10 mb-6 subtle-shadow">
                <div className={`w-2 h-2 rounded-full ${chainInfo ? 'bg-success animate-pulse' : 'bg-muted'}`} />
                <span
                  className={
                    chainInfo
                      ? 'text-sm text-muted-foreground text-shadow-medium'
                      : 'text-sm text-primary font-medium text-shadow-medium'
                  }
                >
                  {chainInfo ? (
                    `Network Active • Block ${chainInfo.best_height.toLocaleString()} • ${chainInfo.peer_count} Peers`
                  ) : (
                    'Connecting to Network...'
                  )}
                </span>
              </div>
              
              <h1 className="text-5xl md:text-7xl font-bold mb-6 tracking-tight">
                <span className="hero-headline-wrap" data-text="Turn Math Into">
                  <span className="hero-headline-inner">Turn Math Into</span>
                </span>
                <br />
                <span className="hero-headline-wrap" data-text="$BEANS">
                  <span className="hero-headline-inner text-primary">$BEANS</span>
                </span>
              </h1>
              
              <p className="text-xl text-white max-w-3xl mx-auto mb-8 text-shadow-medium leading-relaxed">
                COINjecture pays for hard math on-chain — mine blocks for emission, solve marketplace bounties for escrowed payouts, and turn verified NP work into real token value.
              </p>

              <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 max-w-3xl mx-auto mb-10 text-left">
                <Card className="signal-card border-white/10 bg-background/70 p-4">
                  <Calculator className="h-5 w-5 text-primary mb-2" />
                  <div className="font-semibold text-sm mb-1">Solve</div>
                  <p className="text-xs text-muted-foreground">SubsetSum, SAT, TSP — hard problems, not hash grinding</p>
                </Card>
                <Card className="signal-card border-white/10 bg-background/70 p-4">
                  <CheckCircle2 className="h-5 w-5 text-primary mb-2" />
                  <div className="font-semibold text-sm mb-1">Verify</div>
                  <p className="text-xs text-muted-foreground">Solutions checked on-chain in seconds</p>
                </Card>
                <Card className="signal-card border-white/10 bg-background/70 p-4">
                  <Coins className="h-5 w-5 text-primary mb-2" />
                  <div className="font-semibold text-sm mb-1">Earn</div>
                  <p className="text-xs text-muted-foreground">Block rewards + bounty payouts in $BEANS</p>
                </Card>
              </div>

              <div className="flex flex-wrap gap-4 justify-center mb-12">
                <Link to="/solver-lab">
                  <Button size="lg" className="glow-hover gentle-animation px-8">
                    Start Earning <ArrowRight className="ml-2 h-4 w-4" />
                  </Button>
                </Link>
                <Link to="/bounty-submit">
                  <Button size="lg" variant="outline" className="glass-effect border-white/20 gentle-animation px-8 hover:bg-card/50">
                    Post a Bounty <Award className="ml-2 h-4 w-4" />
                  </Button>
                </Link>
                <a href="/COINjecture-Whitepaper.pdf" target="_blank" rel="noopener noreferrer">
                  <Button
                    size="lg"
                    variant="outline"
                    className="gap-2 gentle-animation glass-effect border-white/30 px-8 hover:bg-card/50"
                  >
                    <Download className="h-4 w-4" />
                    Whitepaper
                  </Button>
                </a>
              </div>

              {/* Quick Stats */}
              <div className="grid grid-cols-2 md:grid-cols-4 gap-4 max-w-4xl mx-auto">
                {chainInfo ? (
                  <>
                    <Card className="signal-card interactive-lift border-white/10 bg-background/70">
                      <div className="signal-kicker">Blocks mined</div>
                      <div className="signal-value text-primary">
                        {chainInfo.best_height.toLocaleString()}
                      </div>
                    </Card>
                    <Card className="signal-card interactive-lift border-white/10 bg-background/70">
                      <div className="signal-kicker">Network peers</div>
                      <div className="signal-value text-primary">{chainInfo.peer_count}</div>
                    </Card>
                    {marketplaceStats ? (
                      <>
                        <Card className="signal-card interactive-lift border-white/10 bg-background/70">
                          <div className="signal-kicker">Open bounties</div>
                          <div className="signal-value text-primary">
                            {marketplaceStats.open_problems}
                          </div>
                        </Card>
                        <Card className="signal-card interactive-lift border-white/10 bg-background/70">
                          <div className="signal-kicker">Bounty pool</div>
                          <div className="signal-value text-primary tabular-nums">
                            {formatBeans(parseBalance(marketplaceStats.total_bounty_pool) ?? 0n)}
                          </div>
                        </Card>
                      </>
                    ) : (
                      <>
                        <Card className="signal-card interactive-lift border-white/10 bg-background/70">
                          <div className="flex items-center justify-center h-8">
                            <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                          </div>
                          <div className="signal-kicker mt-2">Loading</div>
                        </Card>
                        <Card className="signal-card interactive-lift border-white/10 bg-background/70">
                          <div className="flex items-center justify-center h-8">
                            <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                          </div>
                          <div className="signal-kicker mt-2">Loading</div>
                        </Card>
                      </>
                    )}
                  </>
                ) : (
                  <>
                    <Card className="signal-card interactive-lift border-white/10 bg-background/70">
                      <div className="flex items-center justify-center h-8">
                        <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                      </div>
                      <div className="signal-kicker mt-2">Loading</div>
                    </Card>
                    <Card className="signal-card interactive-lift border-white/10 bg-background/70">
                      <div className="flex items-center justify-center h-8">
                        <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                      </div>
                      <div className="signal-kicker mt-2">Loading</div>
                    </Card>
                    <Card className="signal-card interactive-lift border-white/10 bg-background/70">
                      <div className="flex items-center justify-center h-8">
                        <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                      </div>
                      <div className="signal-kicker mt-2">Loading</div>
                    </Card>
                    <Card className="signal-card interactive-lift border-white/10 bg-background/70">
                      <div className="flex items-center justify-center h-8">
                        <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                      </div>
                      <div className="signal-kicker mt-2">Loading</div>
                    </Card>
                  </>
                )}
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* The Innovation / How it works Section */}
      <section className="pt-20 pb-8 relative">
        <div className="container mx-auto px-6">
          <div className="max-w-4xl mx-auto">
            <div className="text-center mb-12">
              <p className="text-sm font-semibold uppercase tracking-[0.2em] text-primary mb-3">
                MATH → MONEY
              </p>
              <h2 className="text-4xl md:text-5xl font-bold text-foreground">
                Two ways hard math pays
              </h2>
              <p className="text-muted-foreground mt-4 max-w-2xl mx-auto">
                Unlike chains that burn electricity on meaningless hashes, COINjecture ties security and payouts to verified NP work.
              </p>
            </div>

            <Card className="glass-effect p-8 mb-8">
              <div className="space-y-6">
                <div>
                  <h3 className="text-2xl font-bold mb-3 text-primary">Mine blocks → mint $BEANS</h3>
                  <p className="text-muted-foreground leading-relaxed">
                    Every block requires a verifiable NP solution. Miners earn emission rewards scaled by work score — the harder the math relative to verification time, the more you can mint.
                  </p>
                </div>

                <div>
                  <h3 className="text-2xl font-bold mb-3 text-primary">Solve bounties → claim escrow</h3>
                  <p className="text-muted-foreground leading-relaxed">
                    Anyone can post a problem with locked $BEANS. Submit the winning solution and the bounty settles on-chain — math problems become a live marketplace for payouts.
                  </p>
                </div>

                <div>
                  <h3 className="text-2xl font-bold mb-3 text-primary">Why $BEANS has value</h3>
                  <p className="text-muted-foreground leading-relaxed">
                    Emission follows{" "}
                    <code className="text-xs">⌊w·S·K / isqrt(W)⌋</code> atoms (1 BEANS = 10¹² atoms). Work score{" "}
                    <code className="text-xs">w</code> comes from solve/verify asymmetry — you are paid for computation that is hard to find but cheap to check.
                  </p>
                </div>
              </div>
            </Card>

            <div className="grid md:grid-cols-2 gap-4 max-w-3xl mx-auto">
              <Card className="p-6 glass-effect text-center">
                <Target className="h-10 w-10 text-primary mx-auto mb-3" />
                <h4 className="font-semibold mb-2">For solvers</h4>
                <p className="text-sm text-muted-foreground">
                  Turn algorithm skill into on-chain income — mine the chain or hunt open bounties
                </p>
              </Card>
              <Card className="p-6 glass-effect text-center">
                <Award className="h-10 w-10 text-primary mx-auto mb-3" />
                <h4 className="font-semibold mb-2">For buyers</h4>
                <p className="text-sm text-muted-foreground">
                  Post a bounty, lock $BEANS in escrow, and pay only when a solution verifies
                </p>
              </Card>
            </div>
          </div>
        </div>
      </section>

      {/* NP / PoUW / Security / Marketplace */}
      <section className="pt-10 pb-20 relative bg-muted/30">
        <div className="container mx-auto px-6">
          <div className="max-w-5xl mx-auto">
            <div className="grid md:grid-cols-2 gap-8">
              <Card className="glass-effect p-6">
                <h3 className="text-xl font-semibold mb-3 text-primary">Hard to solve, cheap to verify</h3>
                <p className="text-muted-foreground leading-relaxed">
                  NP asymmetry secures the chain and sets your payout — the same property that makes math valuable makes it mineable.
                </p>
              </Card>

              <Card className="glass-effect p-6">
                <h3 className="text-xl font-semibold mb-3 text-primary">Proof of Useful Work</h3>
                <p className="text-muted-foreground leading-relaxed">
                  Blocks commit to real problem instances. Reveal a valid solution, earn work score, mint $BEANS — no SHA-256 lottery.
                </p>
              </Card>

              <Card className="glass-effect p-6">
                <h3 className="text-xl font-semibold mb-3 text-primary">Open marketplace</h3>
                <p className="text-muted-foreground leading-relaxed">
                  Post problems with escrowed bounties or compete for block rewards. Supply meets demand on-chain.
                </p>
              </Card>

              <Card className="glass-effect p-6">
                <h3 className="text-xl font-semibold mb-3 text-primary">Public proof of output</h3>
                <p className="text-muted-foreground leading-relaxed">
                  Verified solutions stream to Hugging Face — auditable math that backed real payouts.
                </p>
              </Card>
            </div>

            <div className="mt-12 text-center">
              <Card className="glass-effect p-8 inline-block">
                <div className="flex items-center gap-4">
                  <Database className="h-12 w-12 text-primary" />
                  <div className="text-left">
                    <div className="font-semibold mb-1">Transparent & Open</div>
                    <p className="text-sm text-muted-foreground mb-2">
                      All solutions and metrics are publicly available on HuggingFace
                    </p>
                    <a 
                      href={hfDatasetPageUrl()}
                      target="_blank" 
                      rel="noopener noreferrer"
                      className="text-sm text-primary hover:underline inline-flex items-center gap-1"
                    >
                      View Dataset <ArrowRight className="h-3 w-3" />
                    </a>
                  </div>
                </div>
              </Card>
            </div>
          </div>
        </div>
      </section>

      {/* CTA Section */}
      <section id="get-started" className="py-20 relative scroll-mt-24">
        <div className="container mx-auto px-6">
          <div className="max-w-3xl mx-auto text-center">
            <h2 className="text-4xl font-bold mb-4">Get Started</h2>
            <p className="text-lg text-muted-foreground mb-8 leading-relaxed">
              Pick how you want math to pay — solve, explore, or run infrastructure.
            </p>

            <Tabs defaultValue="solve" className="text-left">
              <TabsList className="grid w-full grid-cols-3 h-auto rounded-2xl border border-border/60 bg-muted/30 p-1.5">
                <TabsTrigger
                  value="solve"
                  className="rounded-xl px-3 py-3 text-sm font-semibold data-[state=active]:bg-background data-[state=active]:shadow-sm"
                >
                  Solver Lab
                </TabsTrigger>
                <TabsTrigger
                  value="explore"
                  className="rounded-xl px-3 py-3 text-sm font-semibold data-[state=active]:bg-background data-[state=active]:shadow-sm"
                >
                  Explorer
                </TabsTrigger>
                <TabsTrigger
                  value="node"
                  className="rounded-xl px-3 py-3 text-sm font-semibold data-[state=active]:bg-background data-[state=active]:shadow-sm"
                >
                  Run a node
                </TabsTrigger>
              </TabsList>

              <TabsContent value="solve">
                <Card className="glass-effect p-6 mt-4">
                  <p className="text-muted-foreground mb-4">
                    Mine blocks and claim bounties — turn NP solutions into $BEANS from the browser.
                  </p>
                  <Link to="/solver-lab">
                    <Button size="lg" className="glow-hover gentle-animation w-full sm:w-auto">
                      Open Solver Lab <Code className="ml-2 h-4 w-4" />
                    </Button>
                  </Link>
                </Card>
              </TabsContent>

              <TabsContent value="explore">
                <Card className="glass-effect p-6 mt-4">
                  <p className="text-muted-foreground mb-4">
                    Browse blocks, transactions, and marketplace activity on the live chain.
                  </p>
                  <Link to="/explore">
                    <Button size="lg" variant="outline" className="gentle-animation w-full sm:w-auto">
                      Open Explorer <TrendingUp className="ml-2 h-4 w-4" />
                    </Button>
                  </Link>
                </Card>
              </TabsContent>

              <TabsContent value="node">
                <Card className="glass-effect p-6 mt-4">
                  <p className="text-muted-foreground mb-4">
                    Join the mesh with a prebuilt GHCR image — no local Rust build required.
                  </p>
                  <pre className="bg-terminal-bg text-terminal-text p-4 rounded-lg overflow-x-auto text-xs terminal-font mb-4 whitespace-pre-wrap">
{`docker pull ghcr.io/coinjecture-network/coinjecture2.0:latest
docker compose pull bootnode api-server
docker compose up -d --no-build bootnode api-server`}
                  </pre>
                  <Link to="/api#run-a-node">
                    <Button size="lg" variant="outline" className="gentle-animation w-full sm:w-auto">
                      Node setup docs <Server className="ml-2 h-4 w-4" />
                    </Button>
                  </Link>
                </Card>
              </TabsContent>
            </Tabs>
          </div>
        </div>
      </section>
    </>
  );
};
