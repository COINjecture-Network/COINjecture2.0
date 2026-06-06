#!/usr/bin/env python3
"""Plot COINjecture emission decay: w/W vs w/√W (display BEANS) to 100k blocks."""

from __future__ import annotations

import math
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "docs" / "charts" / "emission_decay.html"

S = 10**12
W_CONST = 10  # typical truncated work score per block
K_VALUES = (50, 20, 9)
MILESTONES = (10_000, 100_000)
MAX_BLOCKS = 100_000


def mint_beans_linear(w: int, w_parent: int, k: int, s: int = S) -> float:
    if w_parent <= 0 or w <= 0:
        return 0.0
    return (w * s * k) // w_parent / s


def mint_beans_sqrt(w: int, w_parent: int, k: int, s: int = S) -> float:
    if w_parent <= 0 or w <= 0:
        return 0.0
    denom = max(1, math.isqrt(w_parent))
    return (w * s * k) // denom / s


def simulate_o1(
    blocks: int,
    w: int,
    k: int,
    sqrt_law: bool,
) -> tuple[list[float], list[float], float]:
    """Returns (per_block, cumulative, lifetime_avg). O(n) — no per-step sum(ws)."""
    w_parent = 0
    per: list[float] = []
    total = 0.0
    mint_fn = mint_beans_sqrt if sqrt_law else mint_beans_linear
    for _ in range(blocks):
        r = mint_fn(w, w_parent, k)
        w_parent += w
        total += r
        per.append(r)
    return per, [0.0] * 0, total / blocks if blocks else 0.0


def simulate_full(
    blocks: int,
    w: int,
    k: int,
    sqrt_law: bool,
) -> tuple[list[int], list[float], list[float]]:
    w_parent = 0
    per: list[float] = []
    cum: list[float] = []
    total = 0.0
    mint_fn = mint_beans_sqrt if sqrt_law else mint_beans_linear
    heights: list[int] = []
    for h in range(1, blocks + 1):
        r = mint_fn(w, w_parent, k)
        w_parent += w
        total += r
        per.append(r)
        cum.append(total)
        heights.append(h)
    return heights, per, cum


def milestone_table() -> str:
    rows = []
    _, lin_per, lin_cum = simulate_full(MAX_BLOCKS, W_CONST, 50, sqrt_law=False)
    rows.append(
        f"<tr><td><b>w/W, K=50</b> (v3 law)</td>"
        f"<td>{lin_per[MILESTONES[0] - 1]:,.4f}</td><td>{lin_cum[MILESTONES[0] - 1]:,.0f}</td>"
        f"<td>{lin_per[MILESTONES[1] - 1]:,.4f}</td><td>{lin_cum[MILESTONES[1] - 1]:,.0f}</td>"
        f"<td>{lin_cum[-1] / MAX_BLOCKS:,.4f}</td></tr>"
    )
    for k in K_VALUES:
        _, sq_per, sq_cum = simulate_full(MAX_BLOCKS, W_CONST, k, sqrt_law=True)
        rows.append(
            f"<tr><td><b>w/√W, K={k}</b></td>"
            f"<td>{sq_per[MILESTONES[0] - 1]:,.4f}</td><td>{sq_cum[MILESTONES[0] - 1]:,.0f}</td>"
            f"<td>{sq_per[MILESTONES[1] - 1]:,.4f}</td><td>{sq_cum[MILESTONES[1] - 1]:,.0f}</td>"
            f"<td>{sq_cum[-1] / MAX_BLOCKS:,.4f}</td></tr>"
        )
    return f"""
<section>
  <h2>Milestones @ block 10,000 and 100,000</h2>
  <p class="note">Constant <code>w_trunc={W_CONST}</code>, <code>S=10¹²</code>, integer floor mint.
  v4 target: <b>w/√W, K=50</b> → lifetime avg ≈ 1 BEANS/block through block 100k.</p>
  <table>
    <thead><tr>
      <th>Scenario</th>
      <th>Per-block @10k</th><th>Cum @10k</th>
      <th>Per-block @100k</th><th>Cum @100k</th>
      <th>Avg 1..100k</th>
    </tr></thead>
    <tbody>{''.join(rows)}</tbody>
  </table>
</section>
"""


def svg_path(
    xs: list[float],
    ys: list[float],
    width: int,
    height: int,
    pad: int,
    y_max: float | None = None,
    log_y: bool = False,
) -> str:
    if not xs:
        return ""
    x_min, x_max = min(xs), max(xs)
    plot_ys = [math.log10(max(y, 1e-12)) for y in ys] if log_y else ys
    y_min = min(plot_ys)
    y_max_plot = max(plot_ys) if y_max is None else (math.log10(y_max) if log_y else y_max)
    if y_max_plot == y_min:
        y_max_plot = y_min + 1

    def px(x: float) -> float:
        return pad + (x - x_min) / max(x_max - x_min, 1) * (width - 2 * pad)

    def py(y: float) -> float:
        v = math.log10(max(y, 1e-12)) if log_y else y
        lo = y_min if log_y else 0.0
        hi = y_max_plot
        return height - pad - (v - lo) / max(hi - lo, 1e-9) * (height - 2 * pad)

    return " ".join(f"{px(xs[i]):.1f},{py(ys[i]):.1f}" for i in range(len(xs)))


def milestone_vlines(width: int, height: int, pad: int, x_max: float) -> str:
    out = ""
    for m in MILESTONES:
        if m > x_max:
            continue
        x = pad + (m / x_max) * (width - 2 * pad)
        out += f'<line x1="{x:.1f}" y1="{pad}" x2="{x:.1f}" y2="{height - pad}" stroke="#f59e0b" stroke-dasharray="4" opacity="0.7"/>'
        out += f'<text x="{x:.1f}" y="{pad - 6}" fill="#fbbf24" font-size="9" text-anchor="middle">h={m // 1000}k</text>'
    return out


def chart_multi_sqrt(
    heights: list[int],
    series: dict[int, list[float]],
    width: int = 920,
    height: int = 400,
    blocks_show: int = MAX_BLOCKS,
    log_y: bool = True,
) -> str:
    pad = 56
    hs = heights[:blocks_show]
    colors = {50: "#4ade80", 20: "#38bdf8", 9: "#a78bfa"}
    polylines = ""
    for k in K_VALUES:
        ys = series[k][:blocks_show]
        pts = svg_path([float(h) for h in hs], ys, width, height, pad, log_y=log_y)
        polylines += f'<polyline fill="none" stroke="{colors[k]}" stroke-width="2" points="{pts}"/>'

    vlines = milestone_vlines(width, height, pad, hs[-1])
    scale = "log Y" if log_y else "linear Y"
    legend = " ".join(
        f'<span style="color:{colors[k]}">━ w/√W K={k}</span>' for k in K_VALUES
    )
    return f"""
<section>
  <h2>w/√W emission ({scale}, blocks 1–{blocks_show:,})</h2>
  <p class="note">{legend}. Orange lines: h=10k and h=100k.</p>
  <svg width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img">
    <rect width="{width}" height="{height}" fill="#0f172a" rx="8"/>
    {vlines}
    {polylines}
    <text x="{pad}" y="20" fill="#94a3b8" font-size="11">BEANS / block</text>
  </svg>
</section>
"""


def chart_compare_tail(
    heights: list[int],
    linear: list[float],
    sqrt50: list[float],
    width: int = 920,
    height: int = 360,
    blocks_show: int = MAX_BLOCKS,
    log_y: bool = True,
) -> str:
    pad = 56
    hs = heights[:blocks_show]
    pts_a = svg_path([float(h) for h in hs], linear[:blocks_show], width, height, pad, log_y=log_y)
    pts_b = svg_path([float(h) for h in hs], sqrt50[:blocks_show], width, height, pad, log_y=log_y)
    vlines = milestone_vlines(width, height, pad, hs[-1])
    return f"""
<section>
  <h2>v3 w/W vs v4 w/√W (K=50, log Y, to {blocks_show:,})</h2>
  <svg width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img">
    <rect width="{width}" height="{height}" fill="#0f172a" rx="8"/>
    {vlines}
    <polyline fill="none" stroke="#38bdf8" stroke-width="2" points="{pts_a}"/>
    <polyline fill="none" stroke="#4ade80" stroke-width="2.5" points="{pts_b}"/>
    <text x="{pad}" y="20" fill="#94a3b8" font-size="11">BEANS / block (log)</text>
  </svg>
  <p class="legend"><span style="color:#38bdf8">━ w/W K=50</span> &nbsp;
  <span style="color:#4ade80">━ w/√W K=50 (v4)</span></p>
</section>
"""


def build_html() -> str:
    heights, per_lin, _ = simulate_full(MAX_BLOCKS, W_CONST, 50, sqrt_law=False)
    sqrt_series: dict[int, list[float]] = {}
    for k in K_VALUES:
        _, per, _ = simulate_full(MAX_BLOCKS, W_CONST, k, sqrt_law=True)
        sqrt_series[k] = per

    return f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8"/>
  <title>COINjecture emission — w/√W (v4) to 100k blocks</title>
  <style>
    body {{ font-family: system-ui, sans-serif; max-width: 980px; margin: 2rem auto; padding: 0 1rem;
           background: #020617; color: #e2e8f0; line-height: 1.5; }}
    h1 {{ font-size: 1.45rem; }}
    h2 {{ font-size: 1.1rem; margin: 0 0 0.5rem; }}
    section {{ margin: 1.75rem 0; padding: 1.1rem; background: #1e293b; border-radius: 12px; }}
    .note {{ color: #94a3b8; font-size: 0.92rem; }}
    code {{ background: #334155; padding: 0.1rem 0.35rem; border-radius: 4px; }}
    table {{ border-collapse: collapse; width: 100%; font-size: 0.88rem; }}
    th, td {{ border: 1px solid #334155; padding: 0.4rem 0.5rem; text-align: right; }}
    th:first-child, td:first-child {{ text-align: left; }}
    .formula {{ background: #0f172a; padding: 1rem; border-radius: 8px; font-family: ui-monospace, monospace; margin: 1rem 0; }}
  </style>
</head>
<body>
  <h1>Emission decay — v4 w/√W (to block 100,000)</h1>
  <p class="note">Example <code>w_trunc={W_CONST}</code>, <code>S=10¹²</code>. Regenerate: <code>python3 scripts/emission_decay_chart.py</code></p>
  <div class="formula">
    <b>v3:</b> mint = ⌊ w·S·K / W ⌋ &nbsp;|&nbsp;
    <b>v4:</b> mint = ⌊ w·S·K / isqrt(W) ⌋ &nbsp;→&nbsp; BEANS ≈ K·w/√W
  </div>
  {milestone_table()}
  {chart_compare_tail(heights, per_lin, sqrt_series[50], blocks_show=MAX_BLOCKS)}
  {chart_multi_sqrt(heights, sqrt_series, blocks_show=MAX_BLOCKS)}
  {chart_multi_sqrt(heights, sqrt_series, blocks_show=10_000, log_y=False)}
</body>
</html>
"""


def main() -> None:
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(build_html(), encoding="utf-8")
    print(f"Wrote {OUT}")
    print("\nMilestone summary (w=10):")
    print(f"{'Scenario':<16} {'@10k blk':>10} {'cum@10k':>12} {'@100k blk':>10} {'cum@100k':>12} {'avg':>8}")
    _, lin_p, lin_c = simulate_full(MAX_BLOCKS, W_CONST, 50, False)
    print(
        f"{'w/W K=50':<16} {lin_p[9999]:>10.4f} {lin_c[9999]:>12.0f} "
        f"{lin_p[99999]:>10.4f} {lin_c[99999]:>12.0f} {lin_c[-1]/MAX_BLOCKS:>8.4f}"
    )
    for k in K_VALUES:
        _, sq_p, sq_c = simulate_full(MAX_BLOCKS, W_CONST, k, True)
        print(
            f"{f'w/√W K={k}':<16} {sq_p[9999]:>10.4f} {sq_c[9999]:>12.0f} "
            f"{sq_p[99999]:>10.4f} {sq_c[99999]:>12.0f} {sq_c[-1]/MAX_BLOCKS:>8.4f}"
        )


if __name__ == "__main__":
    main()
