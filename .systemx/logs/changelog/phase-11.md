# Phase 11 — Docker & Deployment Security

**Date:** 2026-03-25
**Branch:** claude/amazing-perlman

---

## Summary

Hardened all Docker and deployment artifacts to production-grade security standards.

---

## Changes

### 1. Non-root Docker user (`Dockerfile`)

- Added `coinject` user/group (uid/gid `10001`) to the runtime stage.
- `/data` directory created and `chown`-ed to `coinject:coinject` at image build time.
- `USER coinject` instruction drops privileges before `ENTRYPOINT`.
- Avoids running the node process as root, limiting blast radius of any container escape.

### 2. Multi-stage build (already present — enhanced)

- Builder stage: `rust:1.88-slim` — compiles the `coinject` binary.
- Runtime stage: `debian:bookworm-slim` — contains only the binary + runtime libs (`libssl3`, `curl`, `ca-certificates`).
- No Rust toolchain, Cargo caches, or build artifacts in the shipped image.

### 3. Docker security scanning

- Added instructions to `.systemx/docs/guides/deployment.md` for scanning with **Docker Scout** and **Trivy** before any deployment.
- Recommended as a CI step before pushing to a registry.

### 4. Resource limits (`docker-compose.yml`)

All four testnet services now declare:

```yaml
deploy:
  resources:
    limits:
      cpus: '1.0'
      memory: 512M
    reservations:
      cpus: '0.25'
      memory: 128M
```

Production compose uses `cpus: '2.0'` / `memory: 1G` limits.

### 5. Restart policies

- Testnet: `restart: unless-stopped` — restarts on crash but not on explicit `docker compose stop`.
- Production: `restart: always` — restarts on host reboots too.

### 6. Secrets management (`.env.example`, `env_file`)

- Created `.env.example` documenting every required and optional variable.
- `docker-compose.yml` uses `env_file: - .env` so sensitive values (miner address, HF token) are never written into the compose file.
- `.env` is already in `.gitignore` and `.dockerignore`.

### 7. Health checks

- Added `HEALTHCHECK` instruction to `Dockerfile` (polls `http://localhost:9090/health` every 15 s).
- `docker-compose.yml` retains `healthcheck:` blocks on all services; adjusted to `interval: 15s` / `start_period: 40s` to match Dockerfile.

### 8. Network isolation (`docker-compose.yml`, `docker-compose.production.yml`)

Two named networks replace the single `bridge`:

| Network | `internal` | Purpose |
|---|---|---|
| `coinject-internal` | `true` | Node-to-node P2P (port 707). No host routing. |
| `coinject-rpc` | `false` | RPC / metrics port mappings work here. |

Nodes are attached to both. P2P traffic stays on the isolated network; only mapped ports are reachable from the host.

### 9. Volume permissions

- `Dockerfile`: `/data` is `chown coinject:coinject` at build time — named volumes inherit this.
- `deployment.md`: documents `sudo chown -R 10001:10001 /path` for bind-mount host paths.

### 10. Production compose (`docker-compose.production.yml`)

New file with production-appropriate settings:

- RPC (`9933`) and metrics (`9090`) bound to `127.0.0.1` — not directly internet-exposed.
- Port 707 (P2P) still published for peer connectivity.
- Mining disabled by default (validator role).
- `restart: always`.
- JSON-file log driver with `max-size: 50m` / `max-file: 5` rotation.
- Bind-mount volume using `DATA_DIR` env var pointing to a persistent host path.

### 11. Deployment documentation (`.systemx/docs/guides/deployment.md`)

Covers:

- Prerequisites and image scanning
- Environment setup
- Single-node dev mode
- 4-node testnet (`docker-compose.yml`)
- Production deployment (`docker-compose.production.yml`)
- Reverse proxy (Caddy TLS example)
- Firewall rules
- Volume permission setup
- Upgrade procedure
- Troubleshooting table

---

## Files changed

| File | Action |
|---|---|
| `Dockerfile` | Updated — non-root user, HEALTHCHECK, /data ownership |
| `docker-compose.yml` | Updated — resource limits, restart, env_file, dual networks |
| `docker-compose.production.yml` | Created |
| `.env.example` | Created |
| `.systemx/docs/guides/deployment.md` | Created |
| `.systemx/logs/changelog/phase-11.md` | Created (this file) |

---

## Build verification

The Dockerfile was reviewed for correctness. Full `docker build` requires the Rust workspace to be present on a Linux host; the build command is:

```bash
docker build -t coinject-node:phase11 .
```

Expected: binary compiles in builder stage, runtime image runs as uid 10001, `HEALTHCHECK` instruction present in image metadata (`docker inspect coinject-node:phase11 | jq '.[0].Config.Healthcheck'`).
