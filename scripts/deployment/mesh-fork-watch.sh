#!/usr/bin/env bash
# Poll chain_getInfo on several RPC URLs; warn when tips diverge (possible fork / partition).
#
# Usage:
#   ./scripts/deployment/mesh-fork-watch.sh
#   ONE_SHOT=1 ./scripts/deployment/mesh-fork-watch.sh
#   FORK_WATCH_INTERVAL=60 FORK_GAP_WARN=200 FORK_GAP_CRITICAL=3000 \
#   FORK_SPREAD_ALERT_MIN=50000 ./scripts/deployment/mesh-fork-watch.sh
#
# Endpoints: space-separated "label|http://host:port/" (trailing slash optional)
#   FORK_WATCH_TARGETS="a|http://1.2.3.4:9933/ b|http://5.6.7.8:9933/" ./scripts/deployment/mesh-fork-watch.sh
#
# If RPC is not public, open SSH tunnels first, e.g.:
#   ssh -N -L 19933:127.0.0.1:9933 root@HOST &
#   FORK_WATCH_TARGETS="h|http://127.0.0.1:19933/" ONE_SHOT=1 ./scripts/deployment/mesh-fork-watch.sh
#
set -euo pipefail

FORK_WATCH_INTERVAL="${FORK_WATCH_INTERVAL:-120}"
FORK_GAP_WARN="${FORK_GAP_WARN:-150}"
FORK_GAP_CRITICAL="${FORK_GAP_CRITICAL:-2500}"
# Only treat large height **spread** as fork risk if every node is at least this tall
# (avoids CRITICAL during normal resync when one volume was wiped).
FORK_SPREAD_ALERT_MIN="${FORK_SPREAD_ALERT_MIN:-115000}"
ONE_SHOT="${ONE_SHOT:-0}"

FORK_WATCH_TARGETS="${FORK_WATCH_TARGETS:-droplet|http://198.199.81.81:9933/ host1-boot|http://193.203.164.13:9933/ host1-n1|http://193.203.164.13:9934/ host1-n2|http://193.203.164.13:9935/ host1-n3|http://193.203.164.13:9936/ host2-boot|http://76.13.101.67:9933/}"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

export FORK_WATCH_TARGETS FORK_GAP_WARN FORK_GAP_CRITICAL FORK_SPREAD_ALERT_MIN

echo "mesh-fork-watch interval=${FORK_WATCH_INTERVAL}s warn>=${FORK_GAP_WARN} crit>=${FORK_GAP_CRITICAL} spread_alerts_if_min>=${FORK_SPREAD_ALERT_MIN}"
echo "---"

while true; do
  ts="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  out="$(FORK_WATCH_TARGETS="$FORK_WATCH_TARGETS" FORK_GAP_WARN="$FORK_GAP_WARN" FORK_GAP_CRITICAL="$FORK_GAP_CRITICAL" FORK_SPREAD_ALERT_MIN="$FORK_SPREAD_ALERT_MIN" python3 <<'PY'
import json, os, sys, urllib.request

def post(url: str, method: str, params):
    body = json.dumps({"jsonrpc": "2.0", "method": method, "params": params, "id": 1}).encode()
    req = urllib.request.Request(
        url,
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=28) as r:
        return json.loads(r.read().decode())

def hash6(h):
    if isinstance(h, list):
        return "".join(f"{b:02x}" for b in h[:6])
    return str(h)[:12]

rows = []
for raw in os.environ.get("FORK_WATCH_TARGETS", "").split():
    raw = raw.strip()
    if not raw or "|" not in raw:
        continue
    label, url = raw.split("|", 1)
    url = url.strip().rstrip("/") + "/"
    try:
        d = post(url, "chain_getInfo", [])
    except Exception as e:
        rows.append({"label": label, "url": url, "error": str(e)[:100]})
        continue
    if d.get("error"):
        rows.append({"label": label, "url": url, "error": str(d.get("error"))[:100]})
        continue
    r = d.get("result") or {}
    rows.append(
        {
            "label": label,
            "url": url,
            "height": r.get("best_height"),
            "peers": r.get("peer_count"),
            "work": r.get("total_work"),
            "syncing": r.get("is_syncing"),
            "genesis": r.get("genesis_hash"),
            "hash6": hash6(r.get("best_hash")),
        }
    )

warn = int(os.environ.get("FORK_GAP_WARN", "150"))
crit = int(os.environ.get("FORK_GAP_CRITICAL", "2500"))
spread_floor = int(os.environ.get("FORK_SPREAD_ALERT_MIN", "115000"))

lines = []
for r in rows:
    if "error" in r:
        lines.append(f"{r['label']}: ERROR {r['error']}")
    else:
        lines.append(
            f"{r['label']}: h={r['height']} peers={r['peers']} work={r['work']} sync={r['syncing']} tip~{r['hash6']}"
        )

ok_rows = [r for r in rows if "error" not in r and r.get("height") is not None]
verdict = "VERDICT OK"
if ok_rows:
    heights = [int(r["height"]) for r in ok_rows]
    mn, mx = min(heights), max(heights)
    spread = mx - mn
    gens = [r.get("genesis") for r in ok_rows if r.get("genesis")]
    gen_ok = len(set(gens)) <= 1
    spread_mature = mn >= spread_floor
    by_h = {}
    for r in ok_rows:
        by_h.setdefault(int(r["height"]), []).append(r)
    work_fork = any(
        len({x.get("work") for x in lst if x.get("work") is not None}) > 1 for lst in by_h.values()
    )

    header_fork = False
    if gen_ok and mn > 0:
        sigs = []
        for r in ok_rows:
            if int(r["height"]) != mn:
                continue
            try:
                d = post(r["url"], "chain_getBlock", [mn])
                blk = d.get("result") or {}
                h = blk.get("header") or {}
                ph = h.get("prev_hash")
                if isinstance(ph, list):
                    phx = "".join(f"{b:02x}" for b in ph[:8])
                else:
                    phx = str(ph)[:16]
                ch = (h.get("commitment") or {}).get("hash")
                if isinstance(ch, list):
                    chx = "".join(f"{b:02x}" for b in ch[:8])
                else:
                    chx = str(ch)[:16]
                sigs.append(phx + "|" + chx)
            except Exception:
                pass
        if len(set(sigs)) > 1:
            header_fork = True

    parts = [f"spread={spread}", f"min={mn}", f"max={mx}"]
    if not gen_ok:
        verdict = "VERDICT CRITICAL genesis mismatch"
    elif work_fork:
        verdict = "VERDICT CRITICAL same-height different total_work"
    elif header_fork:
        verdict = f"VERDICT CRITICAL different block at h={mn} (fork)"
    elif spread >= crit and spread_mature:
        verdict = f"VERDICT CRITICAL height spread {spread} (>={crit})"
    elif spread >= warn and spread_mature:
        verdict = f"VERDICT WARN height spread {spread} (>={warn})"
    else:
        if spread >= warn and not spread_mature:
            verdict = f"VERDICT CATCH-UP spread={spread} (min={mn}; spread alerts if min>={spread_floor})"
        else:
            verdict = f"VERDICT OK spread={spread}"

print("\n".join(lines))
print(verdict)
PY
)"
  while IFS= read -r line; do
    echo "[$ts] $line"
  done <<<"$out"

  if [[ "$ONE_SHOT" == 1 ]]; then
    exit 0
  fi
  sleep "$FORK_WATCH_INTERVAL"
done
