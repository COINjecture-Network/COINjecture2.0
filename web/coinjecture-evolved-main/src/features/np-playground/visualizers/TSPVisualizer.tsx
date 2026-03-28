type Props = {
  cities: number;
  distances: number[][];
  tour: number[] | null;
};

export function TSPVisualizer({ cities, distances, tour }: Props) {
  if (cities === 0) {
    return <p className="text-sm text-muted-foreground">No cities</p>;
  }

  const maxD = Math.max(...distances.flat(), 1);
  const tourLen =
    tour && tour.length === cities
      ? tour.reduce((acc, _city, i) => {
          const a = tour[i];
          const b = tour[(i + 1) % cities];
          return acc + distances[a][b];
        }, 0)
      : null;

  const vb = 120;
  const cx = (i: number) => 60 + 48 * Math.cos((2 * Math.PI * i) / cities - Math.PI / 2);
  const cy = (i: number) => 60 + 48 * Math.sin((2 * Math.PI * i) / cities - Math.PI / 2);

  const pathD =
    tour && tour.length === cities
      ? tour
          .map((city, i) => {
            const x = cx(city);
            const y = cy(city);
            return `${i === 0 ? "M" : "L"} ${x.toFixed(1)} ${y.toFixed(1)}`;
          })
          .join(" ") +
        ` L ${cx(tour[0]).toFixed(1)} ${cy(tour[0]).toFixed(1)}`
      : "";

  return (
    <div className="space-y-3">
      <p className="text-xs text-muted-foreground">
        Distance matrix (RPC / mining format). Tour order is placed on a circle for readability; edge cost comes from the matrix.
        {tourLen !== null && (
          <span className="block mt-1 text-foreground">
            Tour length (sum of matrix edges): <strong>{tourLen}</strong>
          </span>
        )}
      </p>

      <div className="overflow-x-auto rounded-md border border-border/50">
        <table className="w-full text-xs font-mono border-collapse">
          <thead>
            <tr>
              <th className="border border-border/40 p-1 bg-muted/30" />
              {Array.from({ length: cities }, (_, j) => (
                <th key={j} className="border border-border/40 p-1 bg-muted/30 text-center">
                  {j}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {distances.map((row, i) => (
              <tr key={i}>
                <td className="border border-border/40 p-1 bg-muted/30 text-center font-medium">{i}</td>
                {row.map((d, j) => (
                  <td
                    key={j}
                    className="border border-border/40 p-1 text-center"
                    style={{
                      backgroundColor: `hsl(var(--primary) / ${0.08 + (0.35 * d) / maxD})`,
                    }}
                  >
                    {d}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {pathD && (
        <svg viewBox={`0 0 ${vb} ${vb}`} className="w-full max-h-56 border border-border/60 rounded-md bg-muted/10">
          <path
            d={pathD}
            fill="none"
            stroke="hsl(var(--primary))"
            strokeWidth={1.25}
            strokeLinejoin="round"
            strokeLinecap="round"
          />
          {Array.from({ length: cities }, (_, i) => (
            <g key={i}>
              <circle cx={cx(i)} cy={cy(i)} r={3.5} className="fill-primary stroke-background" strokeWidth={1} />
              <text x={cx(i) + 5} y={cy(i) - 5} className="fill-[hsl(var(--foreground))] text-[9px] font-mono">
                {i}
              </text>
            </g>
          ))}
        </svg>
      )}
    </div>
  );
}
