#!/usr/bin/env python3
"""Chart block rewards: mint = floor(w * S * K / W_parent) atoms, display = atoms / S."""

from __future__ import annotations

import math
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "docs" / "charts" / "reward_emission.html"

SCENARIOS = [
    ("Current (S=10¹², K=50)", 10**12, 50),
    ("Proposed (S=10¹⁶, K=50)", 10**16, 50),
    ("Proposed (S=10¹⁶, K=5)", 10**16, 5),
    ("Proposed (S=10¹⁶, K=1)", 10**16, 1),
]

WORK_PROFILES = [
    ("Constant w=10", lambda n: 10),
    ("Constant w=3", lambda n: 3),
    ("Ramp w=n (early boost)", lambda n: max(1, n)),
]


def mint_beans(w: int, w_parent: int, s: int, k: int) -> float:
    if w_parent <= 0 or w <= 0:
        return 0.0
    atoms = (w * s * k) // w_parent
    return atoms / s


def simulate(profile_fn, s: int, k: int, blocks: int = 200) -> list[float]:
    ws: list[int] = []
    rewards: list[float] = []
    for h in range(1, blocks + 1):
        w = profile_fn(h)
        w_parent = sum(ws)
        ws.append(w)
        rewards.append(mint_beans(w, w_parent, s, k))
    return rewards


def cumulative(rewards: list[float]) -> list[float]:
    out: list[float] = []
    t = 0.0
    for r in rewards:
        t += r
        out.append(t)
    return out


def svg_polyline(points: list[tuple[float, float]], width: int, height: int, pad: int = 48) -> str:
    if not points:
        return ""
    xs = [p[0] for p in points]
    ys = [p[1] for p in points]
    min_x, max_x = min(xs), max(xs)
    min_y, max_y = min(ys), max(ys)
    if max_y == min_y:
        max_y = min_y + 1
    if max_x == min_x:
        max_x = min_x + 1

    def sx(x: float) -> float:
        return pad + (x - min_x) / (max_x - min_x) * (width - 2 * pad)

    def sy(y: float) -> float:
        return height - pad - (y - min_y) / (max_y - min_y) * (height - 2 * pad)

    pts = " ".join(f"{sx(x):.1f},{sy(y):.1f}" for x, y in points)
    return f'<polyline fill="none" stroke-width="2" points="{pts}" />'


def build_html() -> str:
    blocks = 200
    width, height = 920, 320
    sections: list[str] = []

    for profile_name, profile_fn in WORK_PROFILES:
        lines_per_scenario: list[str] = []
        colors = ["#2563eb", "#16a34a", "#ea580c", "#9333ea"]
        legend: list[str] = []
        for i, (label, s, k) in enumerate(SCENARIOS):
            rewards = simulate(profile_fn, s, k, blocks)
            first_harvest = rewards[1] if len(rewards) > 1 else 0
            avg_10_100 = sum(rewards[9:100]) / max(1, len(rewards[9:100]))
            points = [(float(n + 1), rewards[n]) for n in range(blocks)]
            color = colors[i % len(colors)]
            lines_per_scenario.append(
                f'<polyline fill="none" stroke="{color}" stroke-width="2" points="'
                + " ".join(
                    f"{48 + (n / (blocks - 1)) * (width - 96):.1f},{height - 48 - (rewards[n] / max(max(rewards[1:50]), 1e-9)) * (height - 96):.1f}"
                    for n in range(blocks)
                )
                + '" />'
            )
            legend.append(
                f"<li><span style='color:{color}'>■</span> {label} — "
                f"block 2 (first harvest): <b>{first_harvest:,.2f}</b> BEANS, "
                f"avg blocks 10–100: <b>{avg_10_100:,.3f}</b> BEANS</li>"
            )

        cum_section = []
        for label, s, k in SCENARIOS[:2]:
            rewards = simulate(profile_fn, s, k, blocks)
            cum = cumulative(rewards)
            cum_section.append(f"<tr><td>{label}</td><td>{cum[-1]:,.1f}</td><td>{rewards[1]:,.2f}</td></tr>")

        sections.append(
            f"""
<section>
  <h2>{profile_name}</h2>
  <p>Per-block display BEANS (blocks 1–{blocks}). Block 1 usually earns 0 (W_parent=0 at genesis).</p>
  <svg width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img">
    <rect x="0" y="0" width="{width}" height="{height}" fill="#0f172a" rx="8"/>
    <text x="48" y="24" fill="#94a3b8" font-size="12">BEANS / block</text>
    {''.join(lines_per_scenario)}
  </svg>
  <ul>{''.join(legend)}</ul>
  <table>
    <thead><tr><th>Scenario</th><th>Total minted ({blocks} blocks)</th><th>Block 2 reward</th></tr></thead>
    <tbody>{''.join(cum_section)}</tbody>
  </table>
</section>
"""
        )

    return f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8"/>
  <title>COINjecture reward emission chart</title>
  <style>
    body {{ font-family: system-ui, sans-serif; max-width: 980px; margin: 2rem auto; padding: 0 1rem; background: #020617; color: #e2e8f0; }}
    h1 {{ font-size: 1.5rem; }}
    section {{ margin: 2rem 0; padding: 1rem; background: #1e293b; border-radius: 12px; }}
    table {{ border-collapse: collapse; width: 100%; margin-top: 1rem; }}
    th, td {{ border: 1px solid #334155; padding: 0.5rem; text-align: left; }}
    code {{ background: #334155; padding: 0.1rem 0.35rem; border-radius: 4px; }}
    .note {{ color: #94a3b8; font-size: 0.95rem; }}
  </style>
</head>
<body>
  <h1>Block reward emission (display BEANS)</h1>
  <p class="note">Formula: <code>mint_atoms = ⌊ w_trunc · S · K / W_parent ⌋</code>, display BEANS = atoms / S.
  First harvest when <code>W_parent = w_trunc</code> (typically block 2): exactly <code>K</code> display BEANS.</p>
  {''.join(sections)}
  <section>
    <h2>Scale change note</h2>
    <p>Increasing S from 10¹² to 10¹⁶ alone does <strong>not</strong> change display BEANS if K is unchanged —
    only ledger atom integers grow. To shrink first harvest, lower K (e.g. K=5 → 5 BEANS on block 2) or add a
    genesis cumulative-work offset so block 2 is not a first-harvest event.</p>
  </section>
</body>
</html>
"""


def main() -> None:
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(build_html(), encoding="utf-8")
    print(f"Wrote {OUT}")

    # Terminal summary for block 2 and block 100
    print("\nPer-block reward snapshot (constant w=10):")
    print(f"{'Scenario':<28} {'Block 2':>12} {'Block 100':>12} {'Total 200':>14}")
    for label, s, k in SCENARIOS:
        r = simulate(lambda n: 10, s, k, 200)
        print(f"{label:<28} {r[1]:>12.2f} {r[99]:>12.4f} {sum(r):>14.1f}")


if __name__ == "__main__":
    main()
