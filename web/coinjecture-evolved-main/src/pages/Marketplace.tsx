import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { toast } from "sonner";
import { Loader2, ExternalLink } from "lucide-react";
import { Navigation } from "@/components/Navigation";
import { Footer } from "@/components/Footer";
import { MarketplaceClient, type CatalogStats, type MarketplaceCatalogEntry } from "@/lib/marketplace-client";
import { rpcClient, type ProblemType } from "@/lib/rpc-client";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

const catalogClient = new MarketplaceClient();

export default function Marketplace() {
  const queryClient = useQueryClient();
  const [submitOpen, setSubmitOpen] = useState(false);
  const [revealOpen, setRevealOpen] = useState(false);

  const catalogQuery = useQuery({
    queryKey: ["marketplace-export", "catalog"],
    queryFn: async () => {
      const [stats, search] = await Promise.all([
        catalogClient.getStats(),
        catalogClient.search({ limit: 25, offset: 0 }),
      ]);
      return { stats, entries: search.entries };
    },
    refetchInterval: 5_000,
    retry: 1,
  });

  const { data, isLoading, isError, error, refetch } = catalogQuery;

  return (
    <div className="min-h-screen">
      <Navigation />
      <main className="container mx-auto space-y-8 px-4 pb-20 pt-28 sm:px-6">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
          <div>
            <h1 className="font-brand text-3xl font-bold tracking-tight">Marketplace</h1>
            <p className="mt-1 max-w-2xl text-sm text-muted-foreground">
              Catalog from the export service (REST). On-chain bounties use the node RPC wizard.
            </p>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button type="button" onClick={() => setSubmitOpen(true)}>
              Submit bounty
            </Button>
            <Button type="button" variant="secondary" onClick={() => setRevealOpen(true)}>
              Reveal problem
            </Button>
          </div>
        </div>

        {isError && (
          <Alert variant="destructive">
            <AlertTitle>Catalog unavailable</AlertTitle>
            <AlertDescription>
              {error instanceof Error ? error.message : String(error)} — ensure the export service is reachable
              (dev: Vite proxy <code className="rounded bg-muted px-1">/marketplace</code> → 8080).
            </AlertDescription>
          </Alert>
        )}

        {isLoading && !data && (
          <div className="flex items-center gap-2 text-muted-foreground">
            <Loader2 className="h-5 w-5 animate-spin" />
            Loading catalog…
          </div>
        )}

        {data && <CatalogDashboard stats={data.stats} entries={data.entries} onRefresh={() => void refetch()} />}

        <SubmitBountyDialog open={submitOpen} onOpenChange={setSubmitOpen} />
        <RevealProblemDialog
          open={revealOpen}
          onOpenChange={setRevealOpen}
          onSuccess={() => {
            void queryClient.invalidateQueries({ queryKey: ["marketplace-export"] });
          }}
        />
      </main>
      <Footer />
    </div>
  );
}

function CatalogDashboard({
  stats,
  entries,
  onRefresh,
}: {
  stats: CatalogStats;
  entries: MarketplaceCatalogEntry[];
  onRefresh: () => void;
}) {
  const types = [
    { name: "SubsetSum", count: stats.by_type.subset_sum },
    { name: "SAT", count: stats.by_type.sat },
    { name: "TSP", count: stats.by_type.tsp },
    { name: "Custom", count: stats.by_type.custom },
  ];
  const maxCount = Math.max(...types.map((t) => t.count), 1);

  return (
    <>
      <div className="flex justify-end">
        <Button type="button" variant="outline" size="sm" onClick={onRefresh}>
          Refresh now
        </Button>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard title="Total problems" value={stats.total_problems.toLocaleString()} subtitle={`Latest height ${stats.latest_height}`} />
        <StatCard title="Avg work score" value={stats.work_score_stats.avg.toFixed(2)} subtitle="Catalog aggregate" />
        <StatCard title="Avg asymmetry" value={`${stats.time_asymmetry_stats.avg.toFixed(1)}×`} subtitle={`Max ${stats.time_asymmetry_stats.max.toFixed(1)}×`} />
        <StatCard title="Total energy" value={`${(stats.total_energy_joules / 1000).toFixed(1)} kJ`} subtitle="Summed estimates" />
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Problem type mix</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          {types.map((t) => (
            <div key={t.name} className="flex items-center gap-3">
              <div className="w-24 text-sm font-medium">{t.name}</div>
              <div className="h-2 flex-1 overflow-hidden rounded-full bg-muted">
                <div
                  className="h-full rounded-full bg-primary transition-all"
                  style={{ width: `${maxCount ? (t.count / maxCount) * 100 : 0}%` }}
                />
              </div>
              <div className="w-14 text-right text-xs text-muted-foreground">
                {stats.total_problems > 0 ? ((t.count / stats.total_problems) * 100).toFixed(0) : 0}%
              </div>
            </div>
          ))}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Recent catalog entries</CardTitle>
          <CardDescription>Exported proofs — identifiers are shortened in the table.</CardDescription>
        </CardHeader>
        <CardContent className="overflow-x-auto">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Block</TableHead>
                <TableHead>Problem ID</TableHead>
                <TableHead>Work</TableHead>
                <TableHead>Quality</TableHead>
                <TableHead>Asymmetry</TableHead>
                <TableHead>Energy (J)</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {entries.map((e) => (
                <TableRow key={e.problem_id}>
                  <TableCell className="tabular-nums">{e.provenance.block_height}</TableCell>
                  <TableCell className="max-w-[10rem] truncate font-mono text-xs">{e.problem_id}</TableCell>
                  <TableCell>{e.metrics.work_score.toFixed(2)}</TableCell>
                  <TableCell>
                    <QualityBadge q={e.metrics.solution_quality} />
                  </TableCell>
                  <TableCell>{e.metrics.time_asymmetry_ratio.toFixed(1)}×</TableCell>
                  <TableCell className="tabular-nums">{e.metrics.energy_estimate_joules.toFixed(1)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </>
  );
}

function StatCard({ title, value, subtitle }: { title: string; value: string; subtitle: string }) {
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardDescription>{title}</CardDescription>
        <CardTitle className="text-2xl tabular-nums">{value}</CardTitle>
      </CardHeader>
      <CardContent className="text-xs text-muted-foreground">{subtitle}</CardContent>
    </Card>
  );
}

function QualityBadge({ q }: { q: number }) {
  const pct = (q * 100).toFixed(1);
  const variant = q >= 0.9 ? "default" : q >= 0.7 ? "secondary" : "destructive";
  return (
    <Badge variant={variant} className="tabular-nums">
      {pct}%
    </Badge>
  );
}

function SubmitBountyDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (o: boolean) => void }) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Submit a bounty</DialogTitle>
          <DialogDescription>
            Signed public / private listings go through the node JSON-RPC flow (wallet + proof material).
          </DialogDescription>
        </DialogHeader>
        <p className="text-sm text-muted-foreground">
          Use the guided wizard to build <code className="rounded bg-muted px-1">marketplace_submit*</code> params and broadcast.
        </p>
        <DialogFooter className="gap-2 sm:justify-between">
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            Close
          </Button>
          <Button type="button" asChild>
            <Link to="/bounty-submit" className="gap-1" onClick={() => onOpenChange(false)}>
              Open bounty wizard
              <ExternalLink className="h-4 w-4" />
            </Link>
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

const exampleProblemJson = JSON.stringify(
  {
    SubsetSum: { numbers: [10, 20, 30, 40, 50], target: 60 },
  },
  null,
  2,
);

function RevealProblemDialog({
  open,
  onOpenChange,
  onSuccess,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  onSuccess: () => void;
}) {
  const [problemId, setProblemId] = useState("");
  const [salt, setSalt] = useState("");
  const [problemJson, setProblemJson] = useState(exampleProblemJson);

  const mutation = useMutation({
    mutationFn: async () => {
      let problem: ProblemType;
      try {
        problem = JSON.parse(problemJson) as ProblemType;
      } catch {
        throw new Error("Problem JSON is invalid");
      }
      if (!problemId.trim()) throw new Error("Problem ID is required");
      if (!salt.trim()) throw new Error("Salt is required");
      const ok = await rpcClient.revealProblem({
        problem_id: problemId.trim(),
        problem: JSON.stringify(problem),
        salt: salt.trim(),
      });
      if (!ok) throw new Error("Node rejected reveal");
    },
    onSuccess: () => {
      toast.success("Reveal accepted");
      onSuccess();
      onOpenChange(false);
      setProblemId("");
      setSalt("");
      setProblemJson(exampleProblemJson);
    },
    onError: (e: Error) => {
      toast.error("Reveal failed", { description: e.message });
    },
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Reveal private problem</DialogTitle>
          <DialogDescription>
            Calls <code className="text-xs">marketplace_revealProblem</code> on the configured RPC with your problem JSON
            and salt.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-4 py-2">
          <div className="space-y-2">
            <Label htmlFor="reveal-id">Problem ID</Label>
            <Input id="reveal-id" value={problemId} onChange={(e) => setProblemId(e.target.value)} placeholder="0x…" className="font-mono text-xs" />
          </div>
          <div className="space-y-2">
            <Label htmlFor="reveal-salt">Salt (hex)</Label>
            <Input id="reveal-salt" value={salt} onChange={(e) => setSalt(e.target.value)} placeholder="0x…" className="font-mono text-xs" />
          </div>
          <div className="space-y-2">
            <Label htmlFor="reveal-json">Problem (JSON)</Label>
            <Textarea
              id="reveal-json"
              value={problemJson}
              onChange={(e) => setProblemJson(e.target.value)}
              rows={10}
              className="font-mono text-xs"
            />
          </div>
        </div>
        <DialogFooter className="gap-2">
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button type="button" disabled={mutation.isPending} onClick={() => mutation.mutate()}>
            {mutation.isPending ? (
              <>
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                Submitting…
              </>
            ) : (
              "Submit reveal"
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
