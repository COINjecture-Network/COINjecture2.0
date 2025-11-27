import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { ArrowRight, Download, Code, Zap, Award, Target, TrendingUp, Database, Loader2 } from "lucide-react";
import { Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { rpcClient } from "@/lib/rpc-client";

export const Hero = () => {
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
      <section className="min-h-screen pt-32 pb-20 relative overflow-hidden">
        <div className="absolute inset-0 bg-background" />
        
        <div className="container mx-auto px-6 relative z-10">
          <div className="max-w-6xl mx-auto">
            <div className="text-center mb-16 animate-fade-in">
              <div className="inline-flex items-center gap-2 px-4 py-2 rounded-full glass-effect mb-6">
                <div className={`w-2 h-2 rounded-full ${chainInfo ? 'bg-success animate-pulse' : 'bg-muted'}`} />
                <span className="text-sm text-muted-foreground">
                  {chainInfo ? (
                    `Network Active • Block ${chainInfo.best_height.toLocaleString()} • ${chainInfo.peer_count} Peers`
                  ) : (
                    'Connecting to Network...'
                  )}
                </span>
              </div>
              
              <h1 className="text-5xl md:text-7xl font-bold mb-6">
                Proof of Computational Work
                <br />
                <span className="text-primary">Layer 1 Blockchain</span>
              </h1>
              
              <p className="text-xl text-muted-foreground max-w-3xl mx-auto mb-8">
                COINjecture Network is a Layer 1 blockchain protocol that uses computational 
                problem-solving for consensus. Earn <span className="text-primary font-semibold">$BEANS</span> tokens 
                by solving NP-complete problems, creating a computational marketplace where mining provides utility beyond network security.
              </p>
              
              <div className="flex flex-wrap gap-4 justify-center mb-12">
                <Link to="/terminal">
                  <Button size="lg" className="glow-hover">
                    Try Terminal <ArrowRight className="ml-2 h-4 w-4" />
                  </Button>
                </Link>
                <Link to="/bounty-submit">
                  <Button size="lg" variant="outline">
                    Submit Bounty <Award className="ml-2 h-4 w-4" />
                  </Button>
                </Link>
                <a href="/COINjecture-Whitepaper.pdf" target="_blank" rel="noopener noreferrer">
                  <Button size="lg" variant="ghost" className="gap-2">
                    <Download className="h-4 w-4" />
                    Whitepaper
                  </Button>
                </a>
              </div>

              {/* Quick Stats */}
              <div className="grid grid-cols-2 md:grid-cols-4 gap-4 max-w-3xl mx-auto">
                {chainInfo ? (
                  <>
                    <Card className="p-4 glass-effect">
                      <div className="text-2xl font-bold text-primary mb-1">
                        {chainInfo.best_height.toLocaleString()}
                      </div>
                      <div className="text-xs text-muted-foreground">Blocks Mined</div>
                    </Card>
                    <Card className="p-4 glass-effect">
                      <div className="text-2xl font-bold text-primary mb-1">{chainInfo.peer_count}</div>
                      <div className="text-xs text-muted-foreground">Network Peers</div>
                    </Card>
                    {marketplaceStats ? (
                      <>
                        <Card className="p-4 glass-effect">
                          <div className="text-2xl font-bold text-primary mb-1">
                            {marketplaceStats.open_problems}
                          </div>
                          <div className="text-xs text-muted-foreground">Open Problems</div>
                        </Card>
                        <Card className="p-4 glass-effect">
                          <div className="text-2xl font-bold text-primary mb-1">
                            {(marketplaceStats.total_bounty_pool / 1e9).toFixed(2)}B
                          </div>
                          <div className="text-xs text-muted-foreground">Bounty Pool</div>
                        </Card>
                      </>
                    ) : (
                      <>
                        <Card className="p-4 glass-effect">
                          <div className="flex items-center justify-center h-8">
                            <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                          </div>
                          <div className="text-xs text-muted-foreground">Loading...</div>
                        </Card>
                        <Card className="p-4 glass-effect">
                          <div className="flex items-center justify-center h-8">
                            <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                          </div>
                          <div className="text-xs text-muted-foreground">Loading...</div>
                        </Card>
                      </>
                    )}
                  </>
                ) : (
                  <>
                    <Card className="p-4 glass-effect">
                      <div className="flex items-center justify-center h-8">
                        <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                      </div>
                      <div className="text-xs text-muted-foreground">Loading...</div>
                    </Card>
                    <Card className="p-4 glass-effect">
                      <div className="flex items-center justify-center h-8">
                        <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                      </div>
                      <div className="text-xs text-muted-foreground">Loading...</div>
                    </Card>
                    <Card className="p-4 glass-effect">
                      <div className="flex items-center justify-center h-8">
                        <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                      </div>
                      <div className="text-xs text-muted-foreground">Loading...</div>
                    </Card>
                    <Card className="p-4 glass-effect">
                      <div className="flex items-center justify-center h-8">
                        <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                      </div>
                      <div className="text-xs text-muted-foreground">Loading...</div>
                    </Card>
                  </>
                )}
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* What is COINjecture Section */}
      <section className="py-20 relative">
        <div className="container mx-auto px-6">
          <div className="max-w-4xl mx-auto">
            <div className="text-center mb-12">
              <h2 className="text-4xl font-bold mb-4">What is COINjecture Network?</h2>
              <p className="text-lg text-muted-foreground">
                A math-backed Layer 1 blockchain protocol that uses computational problem-solving 
                instead of traditional mining
              </p>
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
                    $BEANS tokens are awarded based on the computational work performed. Problem complexity 
                    and solution efficiency determine the reward amount. This creates incentives for 
                    optimization and effective problem-solving.
                  </p>
                </div>
              </div>
            </Card>

            <div className="grid md:grid-cols-3 gap-4">
              <Card className="p-6 glass-effect text-center">
                <Zap className="h-10 w-10 text-primary mx-auto mb-3" />
                <h4 className="font-semibold mb-2">Energy Efficient</h4>
                <p className="text-sm text-muted-foreground">
                  Average 1.86J per solution - lower than traditional mining approaches
                </p>
              </Card>
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

      {/* How It Works Section */}
      <section className="py-20 relative bg-muted/30">
        <div className="container mx-auto px-6">
          <div className="max-w-5xl mx-auto">
            <div className="text-center mb-12">
              <h2 className="text-4xl font-bold mb-4">How It Works</h2>
              <p className="text-lg text-muted-foreground">
                Four steps to start earning $BEANS through computational work
              </p>
            </div>

            <div className="grid md:grid-cols-2 gap-8">
              <Card className="glass-effect p-6">
                <div className="flex items-start gap-4">
                  <div className="w-10 h-10 rounded-full bg-primary/20 flex items-center justify-center flex-shrink-0">
                    <span className="text-xl font-bold text-primary">1</span>
                  </div>
                  <div>
                    <h3 className="text-xl font-semibold mb-2">Pick a Problem</h3>
                    <p className="text-muted-foreground">
                      Browse the marketplace for computational bounties or submit your own problems. 
                      Each problem has a defined reward in $BEANS.
                    </p>
                  </div>
                </div>
              </Card>

              <Card className="glass-effect p-6">
                <div className="flex items-start gap-4">
                  <div className="w-10 h-10 rounded-full bg-primary/20 flex items-center justify-center flex-shrink-0">
                    <span className="text-xl font-bold text-primary">2</span>
                  </div>
                  <div>
                    <h3 className="text-xl font-semibold mb-2">Solve It</h3>
                    <p className="text-muted-foreground">
                      Use your computational resources to find optimal solutions. Our platform tracks 
                      energy efficiency and solution quality.
                    </p>
                  </div>
                </div>
              </Card>

              <Card className="glass-effect p-6">
                <div className="flex items-start gap-4">
                  <div className="w-10 h-10 rounded-full bg-primary/20 flex items-center justify-center flex-shrink-0">
                    <span className="text-xl font-bold text-primary">3</span>
                  </div>
                  <div>
                    <h3 className="text-xl font-semibold mb-2">Submit Solution</h3>
                    <p className="text-muted-foreground">
                      Your solution is verified on-chain. The network validates correctness and 
                      measures the computational work performed.
                    </p>
                  </div>
                </div>
              </Card>

              <Card className="glass-effect p-6">
                <div className="flex items-start gap-4">
                  <div className="w-10 h-10 rounded-full bg-primary/20 flex items-center justify-center flex-shrink-0">
                    <span className="text-xl font-bold text-primary">4</span>
                  </div>
                  <div>
                    <h3 className="text-xl font-semibold mb-2">Earn $BEANS</h3>
                    <p className="text-muted-foreground">
                      Receive BEANS tokens proportional to the work performed. Higher complexity 
                      and better efficiency earn more rewards.
                    </p>
                  </div>
                </div>
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
            <p className="text-lg text-muted-foreground mb-8">
              Join solvers contributing to computational blockchain
            </p>
            <div className="flex flex-wrap gap-4 justify-center">
              <Link to="/terminal">
                <Button size="lg" className="glow-hover">
                  Open Terminal <Code className="ml-2 h-4 w-4" />
                </Button>
              </Link>
              <Link to="/metrics">
                <Button size="lg" variant="outline">
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
