type Props = {
  variables: number;
  clauses: { literals: number[] }[];
  assignment: boolean[] | null;
};

function litLabel(lit: number): string {
  const v = Math.abs(lit);
  return lit > 0 ? `x${v}` : `¬x${v}`;
}

export function SATVisualizer({ variables, clauses, assignment }: Props) {
  return (
    <div className="space-y-3">
      <p className="text-xs text-muted-foreground">
        DIMACS-style literals: k means x<sub>k</sub>, −k means ¬x<sub>k</sub> (matches `core::problem::Clause` / mining SAT).
      </p>
      {assignment && assignment.length === variables ? (
        <ul className="grid grid-cols-2 sm:grid-cols-4 gap-1.5 text-xs font-mono">
          {assignment.map((val, i) => (
            <li
              key={i}
              className={`rounded border px-2 py-1 ${
                val ? "border-primary/50 bg-primary/10" : "border-border bg-muted/30 text-muted-foreground"
              }`}
            >
              x{i + 1}={val ? "1" : "0"}
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-sm text-muted-foreground">No satisfying assignment found (or variables too large for brute force).</p>
      )}
      <div className="max-h-40 overflow-auto rounded-md border border-border/50 bg-muted/10 p-2 text-xs font-mono space-y-1">
        {clauses.map((c, i) => (
          <div key={i} className="text-muted-foreground">
            <span className="text-foreground/70">c{i + 1}: </span>(
            {c.literals.map(litLabel).join(" ∨ ")})
          </div>
        ))}
      </div>
    </div>
  );
}
