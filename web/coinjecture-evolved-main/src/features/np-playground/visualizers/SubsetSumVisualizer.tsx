type Props = {
  numbers: number[];
  /** Selected indices (network `SolutionType.SubsetSum`) */
  selected: number[] | null;
};

export function SubsetSumVisualizer({ numbers, selected }: Props) {
  const set = selected ? new Set(selected) : null;
  const max = Math.max(...numbers.map((n) => Math.abs(n)), 1);

  return (
    <div className="space-y-2">
      <p className="text-xs text-muted-foreground">
        Bar height ∝ value. Highlight = chosen subset (same indices miners prove on-chain).
      </p>
      <div className="flex items-end gap-1 h-32 border border-border/60 rounded-md p-2 bg-muted/20">
        {numbers.map((n, i) => {
          const h = (Math.abs(n) / max) * 100;
          const on = set?.has(i);
          return (
            <div key={i} className="flex-1 min-w-0 flex flex-col justify-end items-center gap-1" title={`[${i}] ${n}`}>
              <div
                className={`w-full max-w-[28px] rounded-t transition-colors ${
                  on ? "bg-primary" : "bg-muted-foreground/40"
                }`}
                style={{ height: `${Math.max(h, 8)}%` }}
              />
              <span className="text-[10px] text-muted-foreground truncate w-full text-center">{n}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
