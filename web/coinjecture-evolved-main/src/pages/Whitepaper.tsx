import { Navigation } from "@/components/Navigation";
import { Footer } from "@/components/Footer";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Download, ExternalLink, GitBranch, Github } from "lucide-react";

export default function Whitepaper() {
  return (
    <div className="min-h-screen">
      <Navigation />
      
      <main className="pt-32 pb-20">
        <div className="container mx-auto px-6 max-w-4xl">
          {/* Header */}
          <div className="text-center mb-12 animate-fade-in">
            <h1 className="text-5xl font-bold mb-4">
              COINjecture Network
            </h1>
            <p className="text-xl text-primary mb-2">
              A Mathematics-Backed Peer-to-Peer Network
            </p>
            <p className="text-muted-foreground">
              Version 2.3, November 14, 2025
            </p>
            
            <div className="flex flex-wrap gap-4 justify-center mt-8">
              <Button size="lg" className="gap-2">
                <Download className="h-4 w-4" />
                <a href="/COINjecture-Whitepaper.pdf" download>Download PDF</a>
              </Button>
              <Button size="lg" variant="outline" className="gap-2">
                <Github className="h-4 w-4" />
                <a href="https://github.com/Quigles1337/COINjecture2.0" target="_blank" rel="noopener noreferrer">
                  View on GitHub
                </a>
              </Button>
            </div>
          </div>

          {/* Abstract */}
          <Card className="glass-effect p-8 mb-8">
            <h2 className="text-2xl font-bold mb-4 gradient-text">Abstract</h2>
            <p className="text-muted-foreground leading-relaxed">
              COINjecture is a purely peer-to-peer network that harnesses the computational asymmetry benefits 
              inherent to NP-complete mathematical problems to replace the traditional proof-of-work hashing 
              mechanism needed to outpace attackers and secure a robust network of digital payments. Chain 
              validity derives from solution correctness and cumulative mathematical work, ensuring robust 
              consensus without centralized validators. The record formed in the process of solving and verifying 
              these NP-complete problems creates a computational marketplace where miners earn rewards for 
              solving computationally hard problems while enabling secure, decentralized transactions.
            </p>
          </Card>

          {/* Code Repositories */}
          <Card className="glass-effect p-8 mb-8">
            <h2 className="text-2xl font-bold mb-4">Code Base</h2>
            <div className="space-y-3">
              <a 
                href="https://github.com/Quigles1337/COINjecture2.0" 
                target="_blank" 
                rel="noopener noreferrer"
                className="flex items-center gap-3 p-3 rounded-lg hover:bg-muted/50 transition-colors group"
              >
                <GitBranch className="h-5 w-5 text-primary" />
                <span className="flex-1 text-foreground group-hover:text-primary transition-colors">
                  Active Testnet
                </span>
                <ExternalLink className="h-4 w-4 text-muted-foreground" />
              </a>
              <a 
                href="https://gitlab.com/Quigles1337/COINjecture2.0" 
                target="_blank" 
                rel="noopener noreferrer"
                className="flex items-center gap-3 p-3 rounded-lg hover:bg-muted/50 transition-colors group"
              >
                <GitBranch className="h-5 w-5 text-primary" />
                <span className="flex-1 text-foreground group-hover:text-primary transition-colors">
                  GitLab Archive
                </span>
                <ExternalLink className="h-4 w-4 text-muted-foreground" />
              </a>
              <a 
                href="https://codeberg.org/Quigles1337/COINjecture2.0" 
                target="_blank" 
                rel="noopener noreferrer"
                className="flex items-center gap-3 p-3 rounded-lg hover:bg-muted/50 transition-colors group"
              >
                <GitBranch className="h-5 w-5 text-primary" />
                <span className="flex-1 text-foreground group-hover:text-primary transition-colors">
                  Codeberg Archive
                </span>
                <ExternalLink className="h-4 w-4 text-muted-foreground" />
              </a>
            </div>
          </Card>

          {/* Key Sections */}
          <div className="space-y-8">
            <Card className="glass-effect p-8">
              <h2 className="text-2xl font-bold mb-4">1. Introduction</h2>
              <p className="text-muted-foreground leading-relaxed mb-4">
                Traditional proof-of-work blockchain systems, like Bitcoin, rely on the Secure Hash Algorithm 
                256-bit (SHA-256) as the mechanism to verify transactions, link blocks, and maintain network 
                integrity and chain immutability. The SHA-256 algorithm is applied to hash a block header 
                repeatedly until one is found with a specific prefix of leading zeros. This hash calculation 
                provides no scientific or mathematical value outside of network security and the computational 
                resources required resulted in the consumption of over 150 TWh in 2024.
              </p>
              <p className="text-muted-foreground leading-relaxed">
                What is needed is a cryptographically secure electronic payment system that generates utility 
                beyond security. Building on Bitcoin's foundation of decentralized consensus through proof-of-work, 
                COINjecture proposes to replace SHA-256 with NP-complete mathematical problem solving that records 
                verifiable, immutable computational data on-chain. The computational work that fuels chain growth 
                provides network consensus and generates economic utility to establish a computational marketplace 
                in which miners are rewarded for solving computationally hard problems.
              </p>
            </Card>

            <Card className="glass-effect p-8">
              <h2 className="text-2xl font-bold mb-4">2. Transactions</h2>
              <p className="text-muted-foreground leading-relaxed">
                COINjecture follows the Standard UTXO model in which we define an electronic coin as a chain of 
                digital signatures. Each owner transfers the coin by digitally signing a hash of the previous 
                transaction and the public key of the next owner. All blocks in our network must contain not 
                just transactions, but also proofs of computational work (e.g., NP-hard problem solutions) that 
                secure consensus and provide economic utility.
              </p>
            </Card>

            <Card className="glass-effect p-8">
              <h2 className="text-2xl font-bold mb-4">3. Timestamp Server</h2>
              <p className="text-muted-foreground leading-relaxed">
                Like Bitcoin, we employ a timestamp server that proves that the data must have existed at the 
                time of announcement. Each timestamp includes the previous timestamp in its hash as well as the 
                computational proofs (problem + solution) of the block, forming a chain.
              </p>
            </Card>

            <Card className="glass-effect p-8">
              <h2 className="text-2xl font-bold mb-4">4. Proof-of-Work</h2>
              <p className="text-muted-foreground leading-relaxed">
                To implement a distributed timestamp server on a peer-to-peer basis, we use a proof-of-work 
                system based on NP-hard problems rather than hash collision. The defining feature of an NP 
                problem is that a candidate solution can be verified as correct, but not necessarily solved, 
                in polynomial time. For instance, a sudoku puzzle may take several minutes to solve, but can 
                be checked for correctness quickly by ensuring no row or column contains two occurrences of 
                the same number.
              </p>
            </Card>

            <Card className="glass-effect p-8">
              <h2 className="text-2xl font-bold mb-4">5. Commitment Protocol</h2>
              <p className="text-muted-foreground leading-relaxed mb-4">
                To prevent grinding or generating many problems to find easy instances, we employ a succinct 
                commit-mine-reveal protocol:
              </p>
              <ol className="list-decimal list-inside space-y-2 text-muted-foreground">
                <li>Miner computes: commitment = H(problem params || salt || H(solution))</li>
                <li>Miner finds header with valid commitment and sufficient work score</li>
                <li>Miner publishes solution bundle, and prove commitment matches</li>
              </ol>
            </Card>
          </div>

          {/* Download CTA */}
          <div className="mt-12 text-center">
            <Card className="glass-effect p-8">
              <h3 className="text-xl font-semibold mb-4">Read the Full Whitepaper</h3>
              <p className="text-muted-foreground mb-6">
                Download the complete whitepaper to learn about network architecture, consensus mechanisms, 
                economic incentives, and technical specifications.
              </p>
              <Button size="lg" className="gap-2">
                <Download className="h-4 w-4" />
                <a href="/COINjecture-Whitepaper.pdf" download>Download Complete PDF</a>
              </Button>
            </Card>
          </div>
        </div>
      </main>

      <Footer />
    </div>
  );
}
