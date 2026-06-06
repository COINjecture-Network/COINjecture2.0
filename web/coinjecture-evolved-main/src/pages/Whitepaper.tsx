import { Navigation } from "@/components/Navigation";
import { Footer } from "@/components/Footer";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Download, ExternalLink, FileText, GitBranch, Github } from "lucide-react";

export default function Whitepaper() {
  return (
    <div className="min-h-screen">
      <Navigation />

      <main className="pt-32 pb-20">
        <div className="container mx-auto px-6 max-w-4xl">
          <div className="text-center mb-12 animate-fade-in">
            <h1 className="text-5xl font-bold mb-4">
              COINjecture <span className="text-primary">Network</span>
            </h1>
            <p className="text-xl text-primary mb-2">A Mathematics-Backed Peer-to-Peer Network</p>
            <p className="text-muted-foreground">Version 2.6, June 5, 2026 · chain v4 (w/√W emission)</p>

            <div className="flex flex-wrap gap-4 justify-center mt-8">
              <Button size="lg" className="gap-2" asChild>
                <a href="/COINjecture-Whitepaper.pdf" download>
                  <Download className="h-4 w-4" />
                  Download PDF
                </a>
              </Button>
              <Button size="lg" variant="outline" className="gap-2" asChild>
                <a href="/Whitepaper.md" target="_blank" rel="noopener noreferrer">
                  <FileText className="h-4 w-4" />
                  Read Markdown
                </a>
              </Button>
              <Button size="lg" variant="outline" className="gap-2" asChild>
                <a
                  href="https://github.com/COINjecture-Network/COINjecture2.0/blob/main/docs/whitepaper/Whitepaper.tex"
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  <Github className="h-4 w-4" />
                  View on GitHub
                </a>
              </Button>
            </div>
          </div>

          <Card className="glass-effect p-8 mb-8">
            <h2 className="text-2xl font-bold mb-4 gradient-text">Abstract</h2>
            <p className="text-muted-foreground leading-relaxed">
              COINjecture is a purely peer-to-peer network that harnesses the solve-verify asymmetry of NP
              problems to replace traditional proof-of-work hashing (e.g. SHA-256) needed to outpace attackers
              and secure digital payments. Chain validity derives from solution correctness and cumulative
              mathematical work, ensuring robust consensus without centralized validators. Difficulty is
              calibrated to instance hardness, not just solution validity, so trivial solutions yield
              negligible rewards. The record formed by solving and verifying NP problems creates a
              computational marketplace where miners earn rewards for useful work while enabling secure,
              decentralized transactions.
            </p>
          </Card>

          <Card className="glass-effect p-8 mb-8">
            <h2 className="text-2xl font-bold mb-4">Emission (v4 — w/√W)</h2>
            <p className="text-muted-foreground leading-relaxed mb-4">
              Block rewards quantify &ldquo;good work&rdquo; proportionally. Minted ledger atoms per block:
            </p>
            <p className="font-mono text-sm bg-muted/50 rounded-lg p-4 mb-4 text-center">
              mint = ⌊ w<sub>trunc</sub> · S · K / isqrt(W<sub>parent</sub>) ⌋
            </p>
            <ul className="list-disc list-inside space-y-2 text-muted-foreground text-sm">
              <li>
                <strong>w<sub>trunc</sub></strong> — integer truncated header work (solve/verify asymmetry ×
                quality)
              </li>
              <li>
                <strong>W<sub>parent</sub></strong> — sum of w<sub>trunc</sub> through the parent block
                (fork-choice weight uses the same W; mint does not feed back into W)
              </li>
              <li>
                <strong>S = 10¹²</strong> atoms per display BEANS · <strong>K = 50</strong> emission multiplier
              </li>
              <li>
                Tail decay is <strong>1/√W</strong> per block (slower than legacy w/W); v4 targets ~1 BEANS/block
                lifetime average to height 100k
              </li>
            </ul>
          </Card>

          <Card className="glass-effect p-8 mb-8">
            <h2 className="text-2xl font-bold mb-4">Code Base</h2>
            <div className="space-y-3">
              <a
                href="https://github.com/COINjecture-Network/COINjecture2.0"
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-3 p-3 rounded-lg hover:bg-muted/50 transition-colors group"
              >
                <GitBranch className="h-5 w-5 text-primary" />
                <span className="flex-1 text-foreground group-hover:text-primary transition-colors">
                  Active Codebase
                </span>
                <ExternalLink className="h-4 w-4 text-muted-foreground" />
              </a>
            </div>
          </Card>

          <div className="space-y-8">
            <Card className="glass-effect p-8">
              <h2 className="text-2xl font-bold mb-4">1. Introduction</h2>
              <p className="text-muted-foreground leading-relaxed mb-4">
                Traditional proof-of-work systems like Bitcoin rely on SHA-256 brute-force hashing. That work
                secures the network but produces no scientific utility. COINjecture replaces hash grinding with
                NP problem solving: blocks carry verifiable solutions whose correctness and cumulative work
                score drive consensus.
              </p>
              <p className="text-muted-foreground leading-relaxed">
                Proof of Useful Work (PoUW) rewards miners proportionally to measured performance — solution
                quality, solve time, and verify time — through the w/√W emission law above.
              </p>
            </Card>

            <Card className="glass-effect p-8">
              <h2 className="text-2xl font-bold mb-4">2. Proof-of-Work &amp; Commitment</h2>
              <p className="text-muted-foreground leading-relaxed mb-4">
                PoUW uses NP hardness rather than hash collision. A salt–commit–mine–reveal protocol binds each
                block to a specific problem instance and prevents grinding easy instances.
              </p>
              <ol className="list-decimal list-inside space-y-2 text-muted-foreground">
                <li>Miner computes: commitment = H(problem params || salt || H(solution))</li>
                <li>Miner finds a header with valid commitment and sufficient work score</li>
                <li>Miner publishes the solution bundle and proves the commitment matches</li>
              </ol>
            </Card>
          </div>

          <div className="mt-12 text-center">
            <Card className="glass-effect p-8">
              <h3 className="text-xl font-semibold mb-4">Read the Full Whitepaper</h3>
              <p className="text-muted-foreground mb-6">
                Download the PDF or open the Markdown export for network architecture, consensus, formal
                verification (Lean 4), and economic incentives.
              </p>
              <div className="flex flex-wrap gap-3 justify-center">
                <Button size="lg" className="gap-2" asChild>
                  <a href="/COINjecture-Whitepaper.pdf" download>
                    <Download className="h-4 w-4" />
                    PDF
                  </a>
                </Button>
                <Button size="lg" variant="outline" className="gap-2" asChild>
                  <a href="/Whitepaper.md" target="_blank" rel="noopener noreferrer">
                    <FileText className="h-4 w-4" />
                    Markdown
                  </a>
                </Button>
              </div>
            </Card>
          </div>
        </div>
      </main>

      <Footer />
    </div>
  );
}
