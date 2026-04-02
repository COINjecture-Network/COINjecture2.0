import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { ArrowRight, Download, Code, Award, Target, TrendingUp, Database, Loader2 } from "lucide-react";
import { Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { rpcClient } from "@/lib/rpc-client";
import { cn } from "@/lib/utils";

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
                <span className="hero-headline-wrap" data-text="Mathematics-Backed">
                  <span className="hero-headline-inner">Mathematics-Backed</span>
                </span>
                <br />
                <span className="hero-headline-wrap" data-text="Peer-to-Peer Network">
                  <span className="hero-headline-inner">Peer-to-Peer Network</span>
                </span>
              </h1>
              
              <p className="text-xl text-white max-w-3xl mx-auto mb-8 text-shadow-medium leading-relaxed">
                COINjecture harnesses the solve-verify asymmetry of NP problems to replace traditional proof-of-work hashing, providing utility beyond network security.
              </p>

              <div className="flex flex-wrap gap-4 justify-center mb-12">
                <Link to="/solver-lab">
                  <Button size="lg" className="glow-hover gentle-animation px-8">
                    Solver Lab <ArrowRight className="ml-2 h-4 w-4" />
                  </Button>
                </Link>
                <Link to="/bounty-submit">
                  <Button size="lg" variant="outline" className="glass-effect border-white/20 gentle-animation px-8 hover:bg-card/50">
                    Submit Bounty <Award className="ml-2 h-4 w-4" />
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
                          <div className="signal-value text-primary">
                            {(marketplaceStats.total_bounty_pool / 1e9).toFixed(2)}B
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
                THE INNOVATION
              </p>
              <h2 className="text-4xl md:text-5xl font-bold text-foreground">
                How COINjecture Works
              </h2>
            </div>

            <Card className="glass-effect p-8 mb-8">
              <div className="space-y-6">
                <div>
                  <h3 className="text-2xl font-bold mb-3 text-primary">Traditional Mining Approach</h3>
                  <p className="text-muted-foreground leading-relaxed">
                    Traditional proof-of-work blockchains consume significant energy solving 
                    arbitrary mathematical puzzles. This computational power provides network security but 
                    produces no additional value.
                  </p>
                </div>

                <div>
                  <h3 className="text-2xl font-bold mb-3 text-primary">COINjecture Network Approach</h3>
                  <p className="text-muted-foreground leading-relaxed">
                    COINjecture Network is a Layer 1 blockchain protocol that directs computational power toward 
                    solving practical NP-complete problems - SubsetSum, Boolean SAT, TSP, and custom problems. 
                    Each verified solution contributes to algorithm research and practical computation through our 
                    autonomous on-chain marketplace with instant bounty payouts.
                  </p>
                </div>

                <div>
                  <h3 className="text-2xl font-bold mb-3 text-primary">How $BEANS Works</h3>
                  <p className="text-muted-foreground leading-relaxed">
                    $BEANS tokens are awarded based on the computational work performed. The solve/verify time
                    asymmetry and solution quality determine the reward amount. This creates incentives for
                    optimization and effective problem-solving.
                  </p>
                </div>
              </div>
            </Card>

            <div className="grid md:grid-cols-2 gap-4 max-w-3xl mx-auto">
              <Card className="p-6 glass-effect text-center">
                <Target className="h-10 w-10 text-primary mx-auto mb-3" />
                <h4 className="font-semibold mb-2">Useful Work</h4>
                <p className="text-sm text-muted-foreground">
                  Every computation solves real problems with practical applications
                </p>
              </Card>
              <Card className="p-6 glass-effect text-center">
                <Award className="h-10 w-10 text-primary mx-auto mb-3" />
                <h4 className="font-semibold mb-2">Fair Rewards</h4>
                <p className="text-sm text-muted-foreground">
                  Dynamic bounties based on problem complexity and solution quality
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
                <h3 className="text-xl font-semibold mb-3 text-primary">NP Problems Replace Hashing</h3>
                <p className="text-muted-foreground leading-relaxed">
                  Instead of brute-forcing SHA-256 hashes, miners solve NP-complete, co-NP-complete, and
                  NP-hard problems — tasks whose solutions can be verified quickly but are computationally
                  hard to find.
                </p>
              </Card>

              <Card className="glass-effect p-6">
                <h3 className="text-xl font-semibold mb-3 text-primary">Proof of Useful Work</h3>
                <p className="text-muted-foreground leading-relaxed">
                  Block validation requires a verifiable solution via a salt-commit-mine-reveal protocol.
                  Work scores are calculated from the solve-verify time asymmetry and solution quality, not
                  arbitrary hash targets.
                </p>
              </Card>

              <Card className="glass-effect p-6">
                <h3 className="text-xl font-semibold mb-3 text-primary">Same Security Guarantees</h3>
                <p className="text-muted-foreground leading-relaxed">
                  The solve-verify asymmetry of NP problems provides security equivalent to traditional
                  mining. Cumulative mathematical work and solution correctness drive consensus — no
                  centralized validators needed.
                </p>
              </Card>

              <Card className="glass-effect p-6">
                <h3 className="text-xl font-semibold mb-3 text-primary">Computational Marketplace</h3>
                <p className="text-muted-foreground leading-relaxed">
                  The network maintains both consensus-generated problems for block rewards and a
                  user-submitted problem pool with escrowed bounties — creating a marketplace for real
                  computational work.
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
                      href="https://huggingface.co/datasets/COINjecture/NP_Solutions" 
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
      <section className="py-20 relative">
        <div className="container mx-auto px-6">
          <div className="max-w-4xl mx-auto text-center">
            <h2 className="text-4xl font-bold mb-4">Get Started</h2>
            <p className="text-lg text-muted-foreground mb-8 leading-relaxed max-w-3xl mx-auto">
              COINjecture is open-source and community-driven. Whether you&apos;re a developer, researcher, or
              enthusiast — there&apos;s a place for you. Join solvers contributing to computational blockchain or
              submit a bounty for problems you need solved.
            </p>
            <div className="flex flex-wrap gap-4 justify-center">
              <Link to="/solver-lab">
                <Button size="lg" className="glow-hover gentle-animation px-8">
                  Open Solver Lab <Code className="ml-2 h-4 w-4" />
                </Button>
              </Link>
              <Link to="/metrics">
                <Button size="lg" variant="outline" className="gentle-animation px-8">
                  View Live Metrics <TrendingUp className="ml-2 h-4 w-4" />
                </Button>
              </Link>
            </div>
          </div>
        </div>
      </section>
    </>
  );
};
