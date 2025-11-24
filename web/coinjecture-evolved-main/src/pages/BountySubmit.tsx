import { Navigation } from "@/components/Navigation";
import { Footer } from "@/components/Footer";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { useToast } from "@/hooks/use-toast";
import { useState } from "react";

const BountySubmit = () => {
  const { toast } = useToast();
  const [formData, setFormData] = useState({
    title: "",
    problemType: "SubsetSum",
    description: "",
    bounty: "",
    minWorkScore: "100",
    submissionMode: "public",
    expirationDays: "30",
    complexity: "medium",
    priority: "standard",
    verificationMethod: "automated",
    aggregationMethod: "best_solution",
    notes: ""
  });

  const TRANSACTION_FEE = 1000; // Fixed transaction fee in BEANS
  const totalRequired = formData.bounty ? parseInt(formData.bounty) + TRANSACTION_FEE : TRANSACTION_FEE;

  // Problem templates with example data
  const problemTemplates = {
    SubsetSum: {
      title: "Large Dataset Subset Sum Challenge",
      description: `**Problem:** Subset Sum (NP-Complete)

**Input Format:**
- Target sum: T (integer)
- Array of integers: [a₁, a₂, ..., aₙ]
- Array size: n elements

**Example Input:**
\`\`\`json
{
  "target": 15,
  "numbers": [3, 34, 4, 12, 5, 2],
  "size": 6
}
\`\`\`

**Expected Output:**
Return a subset of numbers that sum exactly to the target.

**Example Output:**
\`\`\`json
{
  "solution": [3, 12],
  "sum": 15,
  "indices": [0, 3]
}
\`\`\`

**Constraints:**
- 1 ≤ n ≤ 1000
- -10⁶ ≤ aᵢ ≤ 10⁶
- Solution must be exact (not approximate)

**Verification:** Automated - sum of returned subset must equal target`,
      bounty: "50000",
      minWorkScore: "150",
      complexity: "medium"
    },
    TSP: {
      title: "Traveling Salesman Route Optimization",
      description: `**Problem:** Traveling Salesman Problem (NP-Complete)

**Input Format:**
- Number of cities: n
- Distance matrix: n×n matrix where d[i][j] = distance from city i to city j
- Starting city: city_id

**Example Input:**
\`\`\`json
{
  "cities": 5,
  "distances": [
    [0, 10, 15, 20, 25],
    [10, 0, 35, 25, 30],
    [15, 35, 0, 30, 20],
    [20, 25, 30, 0, 15],
    [25, 30, 20, 15, 0]
  ],
  "start_city": 0
}
\`\`\`

**Expected Output:**
Return the shortest tour visiting all cities exactly once and returning to start.

**Example Output:**
\`\`\`json
{
  "tour": [0, 1, 3, 4, 2, 0],
  "total_distance": 95,
  "visited_all": true
}
\`\`\`

**Constraints:**
- 3 ≤ n ≤ 100
- All distances are positive integers
- Triangle inequality may or may not hold
- Must return to starting city

**Verification:** Automated - validate tour completeness and distance calculation`,
      bounty: "100000",
      minWorkScore: "200",
      complexity: "hard"
    },
    SAT: {
      title: "3-SAT Boolean Satisfiability Instance",
      description: `**Problem:** Boolean Satisfiability (SAT/3-SAT) - NP-Complete

**Input Format:**
- Variables: set of boolean variables {x₁, x₂, ..., xₙ}
- Clauses: CNF formula with clauses of up to 3 literals each
- Number of clauses: m

**Example Input:**
\`\`\`json
{
  "variables": 4,
  "clauses": [
    [1, -2, 3],    // (x₁ ∨ ¬x₂ ∨ x₃)
    [-1, 2, -3],   // (¬x₁ ∨ x₂ ∨ ¬x₃)
    [2, 3, 4],     // (x₂ ∨ x₃ ∨ x₄)
    [-2, -3, 4]    // (¬x₂ ∨ ¬x₃ ∨ x₄)
  ]
}
\`\`\`
Note: Positive numbers = variable, negative = negation

**Expected Output:**
Return a satisfying assignment or proof of unsatisfiability.

**Example Output:**
\`\`\`json
{
  "satisfiable": true,
  "assignment": {
    "x1": true,
    "x2": false,
    "x3": true,
    "x4": true
  },
  "verified_clauses": 4
}
\`\`\`

**Constraints:**
- 3 ≤ n ≤ 500 variables
- Each clause has ≤ 3 literals
- CNF (Conjunctive Normal Form) only
- Must satisfy ALL clauses

**Verification:** Automated - evaluate assignment against all clauses`,
      bounty: "75000",
      minWorkScore: "180",
      complexity: "expert"
    }
  };

  const loadTemplate = (problemType: string) => {
    const template = problemTemplates[problemType as keyof typeof problemTemplates];
    if (template) {
      setFormData({
        ...formData,
        title: template.title,
        description: template.description,
        bounty: template.bounty,
        minWorkScore: template.minWorkScore,
        complexity: template.complexity,
        problemType: problemType
      });
      
      toast({
        title: "Template Loaded ✨",
        description: `${problemType} example loaded. Customize as needed.`,
      });
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    
    toast({
      title: "Bounty Submitted! 🎯",
      description: `Problem "${formData.title}" submitted with ${formData.bounty} BEANS escrowed (Mode: ${formData.submissionMode === 'public' ? 'Public' : 'Private/Commitment'})`,
    });

    // Reset form
    setFormData({
      title: "",
      problemType: "SubsetSum",
      description: "",
      bounty: "",
      minWorkScore: "100",
      submissionMode: "public",
      expirationDays: "30",
      complexity: "medium",
      priority: "standard",
      verificationMethod: "automated",
      aggregationMethod: "best_solution",
      notes: ""
    });
  };

  return (
    <div className="min-h-screen">
      <Navigation />
      <main className="pt-32 pb-20">
        <div className="container mx-auto px-6">
          <div className="max-w-4xl mx-auto">
            <div className="text-center mb-12 animate-fade-in">
              <h1 className="text-4xl md:text-5xl font-bold mb-4">
                Submit a <span className="text-primary">Bounty</span>
              </h1>
              <p className="text-lg text-muted-foreground max-w-2xl mx-auto">
                Submit computational problems with escrowed bounties. Solvers earn BEANS automatically upon verification. Set work score requirements and choose public or private submission modes.
              </p>
            </div>

            {/* Escrow Info Banner */}
            <Card className="glass-effect p-6 mb-8 border-primary/50">
              <div className="flex items-start gap-4">
                <div className="w-12 h-12 rounded-full bg-primary/20 flex items-center justify-center flex-shrink-0">
                  <span className="text-2xl">🔒</span>
                </div>
                <div className="flex-1">
                  <h3 className="font-semibold text-lg mb-2">Autonomous Escrow System</h3>
                  <p className="text-sm text-muted-foreground mb-3">
                    Your bounty is locked in escrow when submitted. Valid solutions trigger <span className="text-primary font-semibold">automatic payout</span> to solvers - no manual claim needed. Unsolved problems auto-refund after expiration.
                  </p>
                  <div className="grid grid-cols-3 gap-4 text-xs">
                    <div className="flex items-center gap-2">
                      <div className="w-2 h-2 rounded-full bg-success" />
                      <span className="text-muted-foreground">Instant payout on valid solution</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <div className="w-2 h-2 rounded-full bg-warning" />
                      <span className="text-muted-foreground">Auto-refund if expired</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <div className="w-2 h-2 rounded-full bg-primary" />
                      <span className="text-muted-foreground">Cancel anytime while open</span>
                    </div>
                  </div>
                </div>
              </div>
            </Card>

            <Card className="glass-effect p-8 glow-primary">
              <form onSubmit={handleSubmit} className="space-y-6">
                {/* Submission Mode Selection */}
                <div className="space-y-2">
                  <Label htmlFor="submissionMode">Submission Mode</Label>
                  <select
                    id="submissionMode"
                    value={formData.submissionMode}
                    onChange={(e) => setFormData({ ...formData, submissionMode: e.target.value })}
                    className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                  >
                    <option value="public">Public - Full problem visible to all solvers</option>
                    <option value="private">Private - Commitment-based (ZK proof) until reveal</option>
                  </select>
                  <p className="text-xs text-muted-foreground">
                    {formData.submissionMode === 'public' 
                      ? 'Problem details are immediately visible. Solvers can start working right away.'
                      : 'Problem hidden using commitment scheme. You control when to reveal full details.'}
                  </p>
                </div>

                <div className="space-y-2">
                  <Label htmlFor="title">Problem Title</Label>
                  <Input
                    id="title"
                    value={formData.title}
                    onChange={(e) => setFormData({ ...formData, title: e.target.value })}
                    placeholder="e.g., Optimize Subset Sum for Large Dataset"
                    required
                  />
                </div>

                <div className="grid md:grid-cols-2 gap-6">
                  <div className="space-y-2">
                    <Label htmlFor="problemType">Problem Type</Label>
                    <select
                      id="problemType"
                      value={formData.problemType}
                      onChange={(e) => setFormData({ ...formData, problemType: e.target.value })}
                      className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                    >
                      <option value="SubsetSum">Subset Sum (NP-Complete)</option>
                      <option value="TSP">Traveling Salesman Problem (TSP)</option>
                      <option value="SAT">Boolean Satisfiability (SAT)</option>
                    </select>
                    <div className="flex items-center justify-between mt-2">
                      <p className="text-xs text-muted-foreground">
                        All problems are NP-Complete - verified for computational work proof
                      </p>
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={() => loadTemplate(formData.problemType)}
                        className="text-xs"
                      >
                        Load Example 📝
                      </Button>
                    </div>
                  </div>

                  <div className="space-y-2">
                    <Label htmlFor="complexity">Complexity Level</Label>
                    <select
                      id="complexity"
                      value={formData.complexity}
                      onChange={(e) => setFormData({ ...formData, complexity: e.target.value })}
                      className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                    >
                      <option value="easy">Easy (P complexity)</option>
                      <option value="medium">Medium (NP)</option>
                      <option value="hard">Hard (NP-Hard)</option>
                      <option value="expert">Expert (NP-Complete)</option>
                    </select>
                  </div>
                </div>

                <div className="space-y-2">
                  <Label htmlFor="description">Problem Description</Label>
                  
                  {/* Template Preview */}
                  {!formData.description && (
                    <Card className="p-4 bg-muted/30 border-dashed mb-3">
                      <div className="flex items-start gap-3">
                        <span className="text-2xl">📋</span>
                        <div className="flex-1">
                          <h4 className="font-semibold text-sm mb-1">
                            {formData.problemType} Template Available
                          </h4>
                          <p className="text-xs text-muted-foreground mb-2">
                            Click "Load Example" above to populate with a properly formatted {formData.problemType} problem including input/output examples, constraints, and verification requirements.
                          </p>
                          <div className="text-xs text-muted-foreground space-y-1">
                            <div>• Includes JSON format examples</div>
                            <div>• Shows expected input/output structure</div>
                            <div>• Pre-filled with realistic constraints</div>
                          </div>
                        </div>
                      </div>
                    </Card>
                  )}
                  
                  <Textarea
                    id="description"
                    value={formData.description}
                    onChange={(e) => setFormData({ ...formData, description: e.target.value })}
                    placeholder="Describe the computational problem, constraints, input format, and expected output. Include test cases if available..."
                    className="min-h-[120px] font-mono text-sm"
                    required
                  />
                </div>

                {/* Bounty & Work Score Requirements */}
                <div className="grid md:grid-cols-2 gap-6">
                  <div className="space-y-2">
                    <Label htmlFor="bounty">Bounty Amount (BEANS)</Label>
                    <Input
                      id="bounty"
                      type="number"
                      value={formData.bounty}
                      onChange={(e) => setFormData({ ...formData, bounty: e.target.value })}
                      placeholder="1000000"
                      min="1000"
                      required
                    />
                    <p className="text-xs text-muted-foreground">
                      Minimum: 1,000 BEANS
                    </p>
                  </div>

                  <div className="space-y-2">
                    <Label htmlFor="minWorkScore">Minimum Work Score</Label>
                    <Input
                      id="minWorkScore"
                      type="number"
                      value={formData.minWorkScore}
                      onChange={(e) => setFormData({ ...formData, minWorkScore: e.target.value })}
                      placeholder="100"
                      min="1"
                      required
                    />
                    <p className="text-xs text-muted-foreground">
                      Quality threshold - solutions must meet or exceed this score
                    </p>
                  </div>
                </div>

                {/* Escrow Summary */}
                <Card className="p-4 bg-muted/50 border-primary/20">
                  <div className="space-y-2">
                    <div className="flex justify-between text-sm">
                      <span className="text-muted-foreground">Bounty to escrow:</span>
                      <span className="font-semibold">{formData.bounty || '0'} BEANS</span>
                    </div>
                    <div className="flex justify-between text-sm">
                      <span className="text-muted-foreground">Transaction fee:</span>
                      <span className="font-semibold">{TRANSACTION_FEE.toLocaleString()} BEANS</span>
                    </div>
                    <div className="h-px bg-border my-2" />
                    <div className="flex justify-between">
                      <span className="font-semibold text-primary">Total Required:</span>
                      <span className="font-bold text-primary text-lg">{totalRequired.toLocaleString()} BEANS</span>
                    </div>
                  </div>
                </Card>

                <div className="space-y-2">
                  <Label htmlFor="expirationDays">Expiration Period (Days)</Label>
                  <Input
                    id="expirationDays"
                    type="number"
                    value={formData.expirationDays}
                    onChange={(e) => setFormData({ ...formData, expirationDays: e.target.value })}
                    placeholder="30"
                    min="1"
                    max="365"
                    required
                  />
                  <p className="text-xs text-muted-foreground">
                    Auto-refund if unsolved after this period. Maximum: 365 days
                  </p>
                </div>

                {/* Advanced Options */}
                <div className="grid md:grid-cols-3 gap-6">
                  <div className="space-y-2">
                    <Label htmlFor="priority">Priority</Label>
                    <select
                      id="priority"
                      value={formData.priority}
                      onChange={(e) => setFormData({ ...formData, priority: e.target.value })}
                      className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                    >
                      <option value="low">Low Priority</option>
                      <option value="standard">Standard</option>
                      <option value="high">High Priority</option>
                      <option value="urgent">Urgent</option>
                    </select>
                  </div>

                  <div className="space-y-2">
                    <Label htmlFor="verificationMethod">Verification Method</Label>
                    <select
                      id="verificationMethod"
                      value={formData.verificationMethod}
                      onChange={(e) => setFormData({ ...formData, verificationMethod: e.target.value })}
                      className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                    >
                      <option value="automated">Automated Testing</option>
                      <option value="manual">Manual Review</option>
                      <option value="hybrid">Hybrid Verification</option>
                      <option value="community">Community Voting</option>
                    </select>
                  </div>

                  <div className="space-y-2">
                    <Label htmlFor="aggregationMethod">Aggregation Method</Label>
                    <select
                      id="aggregationMethod"
                      value={formData.aggregationMethod}
                      onChange={(e) => setFormData({ ...formData, aggregationMethod: e.target.value })}
                      className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                    >
                      <option value="best_solution">Best Solution</option>
                      <option value="first_valid">First Valid</option>
                      <option value="consensus">Consensus</option>
                      <option value="weighted_average">Weighted Average</option>
                    </select>
                  </div>
                </div>

                <div className="space-y-2">
                  <Label htmlFor="notes">Additional Notes (Optional)</Label>
                  <Textarea
                    id="notes"
                    value={formData.notes}
                    onChange={(e) => setFormData({ ...formData, notes: e.target.value })}
                    placeholder="Any additional context, special requirements, or clarifications..."
                    className="min-h-[80px]"
                  />
                </div>

                <div className="bg-muted/50 p-4 rounded-lg border border-border/50">
                  <h3 className="font-semibold mb-2 flex items-center gap-2">
                    <span className="text-primary">💡</span> Bounty Lifecycle
                  </h3>
                  <ul className="text-sm text-muted-foreground space-y-1">
                    <li>• <strong>Escrowed:</strong> Bounty locked immediately upon submission</li>
                    <li>• <strong>Auto-payout:</strong> Valid solutions trigger instant payment to solver</li>
                    <li>• <strong>Auto-refund:</strong> Unsolved problems refund after expiration</li>
                    <li>• <strong>Cancellable:</strong> You can cancel open problems anytime</li>
                  </ul>
                </div>

                <Button type="submit" className="w-full" size="lg">
                  Submit Problem & Escrow {formData.bounty || '0'} BEANS
                </Button>
              </form>
            </Card>

            <div className="mt-8 grid md:grid-cols-3 gap-4">
              <Card className="p-4 text-center">
                <div className="text-3xl font-bold text-primary mb-1">156</div>
                <div className="text-sm text-muted-foreground">Active Problems</div>
              </Card>
              <Card className="p-4 text-center">
                <div className="text-3xl font-bold text-primary mb-1">4.73B</div>
                <div className="text-sm text-muted-foreground">Total BEANS Escrowed</div>
              </Card>
              <Card className="p-4 text-center">
                <div className="text-3xl font-bold text-primary mb-1">100%</div>
                <div className="text-sm text-muted-foreground">Auto-payout Rate</div>
              </Card>
            </div>
          </div>
        </div>
      </main>
      <Footer />
    </div>
  );
};

export default BountySubmit;
