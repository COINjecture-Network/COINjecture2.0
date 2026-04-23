#!/usr/bin/env python3
"""DEPRECATED: upstream docker-compose.yml no longer uses network_mode: service:bootnode (stale-netns after bootnode restart).

Do not run this on new deploys. Use repo docker-compose.yml: api-server on coinject-rpc + NODE_RPC_URL=http://bootnode:9933.
Legacy one-shot for old VPS trees that still had the shared-netns layout.
"""
from pathlib import Path
import sys

def main() -> int:
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/opt/coinjecture")
    p = root / "docker-compose.yml"
    t = p.read_text()
    orig = t

    boot = t.split("  bootnode:", 1)[1].split("  node1:", 1)[0]
    if '"3030:3030"' not in boot:
        t = t.replace(
            '- "9933:9933"    # JSON-RPC\n',
            '- "9933:9933"    # JSON-RPC\n      - "3030:3030"\n',
            1,
        )

    replacements = [
        (
            """  api-server:
    build:
      context: .
      dockerfile: Dockerfile.api
    image: coinject-api:latest
    container_name: coinject-api
    extra_hosts:
      - "host.docker.internal:host-gateway"
    ports:
      - "3030:3030"
      - "9094:9090"  # metrics
    environment:
      - COINJECTURE_API_HOST=0.0.0.0
      - COINJECTURE_API_PORT=3030
      - SUPABASE_URL=${SUPABASE_URL}
      - SUPABASE_ANON_KEY=${SUPABASE_ANON_KEY}
      - SUPABASE_JWT_SECRET=${SUPABASE_JWT_SECRET}
      - RUST_LOG=info
      - COINJECTURE_NETWORK=testnet
      - NODE_RPC_URL=${NODE_RPC_URL:-http://host.docker.internal:9933}
    restart: unless-stopped
    networks:
      - coinject-rpc""",
            """  api-server:
    build:
      context: .
      dockerfile: Dockerfile.api
    image: coinject-api:latest
    container_name: coinject-api
    network_mode: "service:bootnode"
    depends_on:
      bootnode:
        condition: service_healthy
    environment:
      - COINJECTURE_API_HOST=0.0.0.0
      - COINJECTURE_API_PORT=3030
      - SUPABASE_URL=${SUPABASE_URL}
      - SUPABASE_ANON_KEY=${SUPABASE_ANON_KEY}
      - SUPABASE_JWT_SECRET=${SUPABASE_JWT_SECRET}
      - RUST_LOG=info
      - COINJECTURE_NETWORK=testnet
      - NODE_RPC_URL=${NODE_RPC_URL:-http://127.0.0.1:9933}
    restart: unless-stopped""",
        ),
        (
            """  api-server:
    build:
      context: .
      dockerfile: Dockerfile.api
    image: coinject-api:latest
    container_name: coinject-api
    extra_hosts:
      - "host.docker.internal:host-gateway"
    ports:
      - "3030:3030"
      - "9094:9090"  # metrics
    environment:
      - COINJECTURE_API_HOST=0.0.0.0
      - COINJECTURE_API_PORT=3030
      - SUPABASE_URL=${SUPABASE_URL}
      - SUPABASE_ANON_KEY=${SUPABASE_ANON_KEY}
      - SUPABASE_JWT_SECRET=${SUPABASE_JWT_SECRET}
      - RUST_LOG=info
      - COINJECTURE_NETWORK=testnet
      - NODE_RPC_URL=${NODE_RPC_URL:-http://bootnode:9933}
    restart: unless-stopped
    networks:
      - coinject-rpc""",
            """  api-server:
    build:
      context: .
      dockerfile: Dockerfile.api
    image: coinject-api:latest
    container_name: coinject-api
    network_mode: "service:bootnode"
    depends_on:
      bootnode:
        condition: service_healthy
    environment:
      - COINJECTURE_API_HOST=0.0.0.0
      - COINJECTURE_API_PORT=3030
      - SUPABASE_URL=${SUPABASE_URL}
      - SUPABASE_ANON_KEY=${SUPABASE_ANON_KEY}
      - SUPABASE_JWT_SECRET=${SUPABASE_JWT_SECRET}
      - RUST_LOG=info
      - COINJECTURE_NETWORK=testnet
      - NODE_RPC_URL=${NODE_RPC_URL:-http://127.0.0.1:9933}
    restart: unless-stopped""",
        ),
    ]

    for old, new in replacements:
        if old in t:
            t = t.replace(old, new, 1)
            break

    if t == orig:
        print("patch-api-network-mode: no changes applied (unexpected compose layout)")
        return 1

    p.write_text(t)
    print("patch-api-network-mode: updated", p)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
