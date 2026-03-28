import { useEffect } from "react";
import { Navigation } from "@/components/Navigation";
import { Footer } from "@/components/Footer";
import { Card } from "@/components/ui/card";
import { cn } from "@/lib/utils";

type Audience = "miners" | "users" | "world" | "ai" | "all";

const audienceBorder: Record<Audience, string> = {
  miners: "border-t-[3px] border-t-accent-purple",
  users: "border-t-[3px] border-t-accent-emerald",
  world: "border-t-[3px] border-t-warning",
  ai: "border-t-[3px] border-t-[#D4537E]",
  all: "border-t-[3px] border-t-muted-foreground/60",
};

const audienceLabel: Record<Audience, string> = {
  miners: "text-accent-purple",
  users: "text-accent-emerald",
  world: "text-warning",
  ai: "text-[#D4537E]",
  all: "text-muted-foreground",
};

type RoadmapCard = {
  audience: Audience;
  audienceLabel: string;
  title: string;
  description: string;
};

type Stage = {
  dot: "active" | "next" | "future";
  name: string;
  time: string;
  tagline: string;
  cards: RoadmapCard[];
};

const stages: Stage[] = [
  {
    dot: "active",
    name: "Now — Proof of concept",
    time: "Testnet · 2026 H1",
    tagline: "Work that means something",
    cards: [
      {
        audience: "miners",
        audienceLabel: "Miners",
        title: "Earn by solving, not guessing",
        description: "First network where mining skill — not hardware luck — determines reward",
      },
      {
        audience: "world",
        audienceLabel: "Research",
        title: "NP solutions recorded on-chain",
        description: "Subset Sum, TSP, and 3-SAT results form a public, verifiable ledger from day one",
      },
      {
        audience: "ai",
        audienceLabel: "AI / ML",
        title: "Ground-truth reasoning data",
        description:
          "Every solved block is a labeled problem-solution pair — difficulty, solve time, and quality all verified on-chain",
      },
      {
        audience: "all",
        audienceLabel: "Everyone",
        title: "Mining energy that pays twice",
        description: "Same electricity — but the output is useful math, not discarded hashes",
      },
    ],
  },
  {
    dot: "next",
    name: "The marketplace opens",
    time: "2026 H2",
    tagline: "Computation becomes a commodity",
    cards: [
      {
        audience: "users",
        audienceLabel: "Users / businesses",
        title: "Pay to solve your hard problems",
        description: "Submit optimization or logistics problems — get solutions back with verified proof of work",
      },
      {
        audience: "miners",
        audienceLabel: "Miners",
        title: "Two income streams, one machine",
        description: "Block rewards and user bounties run simultaneously — skill and speed both rewarded",
      },
      {
        audience: "ai",
        audienceLabel: "AI / ML",
        title: "A live benchmark, not a static test",
        description:
          "As problem difficulty scales with the network, it becomes a continuously updating measure of solver capability — impossible to overfit",
      },
      {
        audience: "world",
        audienceLabel: "Research",
        title: "The conjecture goes on trial",
        description: "Empirical network data validates or falsifies the coherence prediction in real time",
      },
    ],
  },
  {
    dot: "future",
    name: "Mainnet — The full flywheel",
    time: "2027+",
    tagline: "Network effects compound",
    cards: [
      {
        audience: "all",
        audienceLabel: "Everyone",
        title: "Token value tied to real utility",
        description:
          "COIN is backed by computational work done — not speculation. The dimensional pool system aligns time horizon with reward",
      },
      {
        audience: "ai",
        audienceLabel: "AI / ML",
        title: "The world's largest NP dataset",
        description:
          "Millions of economically incentivized solutions across problem types, difficulty levels, and solver strategies — a corpus no lab could generate alone",
      },
      {
        audience: "world",
        audienceLabel: "Research",
        title: "A permanent complexity benchmark",
        description:
          "Every block adds to a public dataset of verified NP solutions — searchable, reproducible, growing forever",
      },
      {
        audience: "users",
        audienceLabel: "Users / businesses",
        title: "Enterprise computation at scale",
        description:
          "Logistics, drug discovery, financial optimization — workloads too hard for classical solvers find a global market of incentivized solvers",
      },
    ],
  },
];

const dotClass = {
  active: "bg-success",
  next: "bg-accent-blue",
  future: "bg-muted-foreground",
} as const;

function RoadmapCardView({ card }: { card: RoadmapCard }) {
  return (
    <Card
      className={cn(
        "glass-effect p-4 border-t-[3px] rounded-xl transition-all hover:-translate-y-px hover:border-border/80",
        audienceBorder[card.audience]
      )}
    >
      <div className={cn("font-mono text-[10px] font-medium uppercase tracking-wider mb-2", audienceLabel[card.audience])}>
        {card.audienceLabel}
      </div>
      <h3 className="text-sm font-medium text-foreground mb-1.5 leading-snug">{card.title}</h3>
      <p className="text-xs text-muted-foreground leading-relaxed font-normal">{card.description}</p>
    </Card>
  );
}

export default function Roadmap() {
  useEffect(() => {
    document.title = "Value Roadmap — COINjecture";
  }, []);

  return (
    <div className="min-h-screen">
      <Navigation />
      <main className="pt-32 pb-20">
        <div className="container mx-auto px-6 max-w-4xl">
          <header className="text-center mb-12">
            <h1 className="text-4xl font-bold mb-4 tracking-tight">
              A roadmap to <span className="text-primary">value creation</span>
            </h1>
            <p className="text-muted-foreground max-w-lg mx-auto leading-relaxed">
              When each group — miners, businesses, researchers, and AI systems — starts seeing real returns from the network.
            </p>
          </header>

          <Card className="glass-effect p-4 md:p-5 mb-10 flex flex-wrap justify-center gap-x-5 gap-y-2.5">
            <LegendSwatch color="bg-accent-purple" label="Miners" />
            <LegendSwatch color="bg-accent-emerald" label="Users / businesses" />
            <LegendSwatch color="bg-warning" label="Research / world" />
            <LegendSwatch color="bg-[#D4537E]" label="AI / ML" />
            <LegendSwatch color="bg-muted-foreground/70" label="Everyone" />
          </Card>

          <div className="space-y-10">
            {stages.map((stage) => (
              <section key={stage.name} className="space-y-4">
                <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1 pb-3 border-b border-border/80">
                  <span className={cn("inline-block h-2 w-2 rounded-full shrink-0 mt-1", dotClass[stage.dot])} aria-hidden />
                  <span className="text-[15px] font-medium text-foreground">{stage.name}</span>
                  <span className="font-mono text-[11px] text-muted-foreground">{stage.time}</span>
                  <span className="hidden sm:block ml-auto text-xs text-muted-foreground italic font-light">{stage.tagline}</span>
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-2.5">
                  {stage.cards.map((c) => (
                    <RoadmapCardView key={c.title} card={c} />
                  ))}
                </div>
              </section>
            ))}
          </div>
        </div>
      </main>
      <Footer />
    </div>
  );
}

function LegendSwatch({ color, label }: { color: string; label: string }) {
  return (
    <div className="flex items-center gap-2 font-mono text-xs text-muted-foreground">
      <span className={cn("h-2.5 w-2.5 rounded-sm shrink-0", color)} aria-hidden />
      <span>{label}</span>
    </div>
  );
}
