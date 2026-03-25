# Dockerfile for COINjecture CPP Network (libp2p removed)
# Multi-stage build: builder compiles, runtime is minimal debian-slim
# Security: runs as non-root user 'coinject' (uid 10001)

# ── Builder stage ────────────────────────────────────────────────────────────
FROM rust:1.88-slim AS builder

# Use kernel.org mirror (deb.debian.org/Fastly CDN unreachable from some Docker networks)
RUN echo 'Types: deb\nURIs: http://mirrors.kernel.org/debian\nSuites: bookworm bookworm-updates\nComponents: main\nSigned-By: /usr/share/keyrings/debian-archive-keyring.gpg' > /etc/apt/sources.list.d/debian.sources

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy Cargo files for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY adzdb/Cargo.toml ./adzdb/
COPY core/Cargo.toml ./core/
COPY consensus/Cargo.toml ./consensus/
COPY network/Cargo.toml ./network/
COPY state/Cargo.toml ./state/
COPY mempool/Cargo.toml ./mempool/
COPY rpc/Cargo.toml ./rpc/
COPY tokenomics/Cargo.toml ./tokenomics/
COPY node/Cargo.toml ./node/
COPY wallet/Cargo.toml ./wallet/
COPY marketplace-export/Cargo.toml ./marketplace-export/
COPY huggingface/Cargo.toml ./huggingface/
COPY mobile-sdk/Cargo.toml ./mobile-sdk/

# Copy source code
COPY adzdb ./adzdb
COPY core ./core
COPY consensus ./consensus
COPY network ./network
COPY state ./state
COPY mempool ./mempool
COPY rpc ./rpc
COPY tokenomics ./tokenomics
COPY node ./node
COPY wallet ./wallet
COPY marketplace-export ./marketplace-export
COPY huggingface ./huggingface
COPY mobile-sdk ./mobile-sdk

# Build release binary
RUN cargo build --release --bin coinject

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

# Use kernel.org mirror (deb.debian.org/Fastly CDN unreachable from some Docker networks)
RUN echo 'Types: deb\nURIs: http://mirrors.kernel.org/debian\nSuites: bookworm bookworm-updates\nComponents: main\nSigned-By: /usr/share/keyrings/debian-archive-keyring.gpg' > /etc/apt/sources.list.d/debian.sources

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user and group
# uid/gid 10001 — avoids collision with common system users
RUN groupadd --gid 10001 coinject \
 && useradd --uid 10001 --gid coinject --shell /sbin/nologin --no-create-home coinject

# Copy binary from builder
COPY --from=builder /build/target/release/coinject /usr/local/bin/coinject

# Create data directory owned by coinject user
RUN mkdir -p /data && chown coinject:coinject /data

# Drop to non-root user for all subsequent instructions and runtime
USER coinject

# Expose CPP network ports
# CPP P2P port (default: 707)
EXPOSE 707
# CPP WebSocket RPC port (default: 8080)
EXPOSE 8080
# JSON-RPC port (default: 9933)
EXPOSE 9933
# Metrics port (default: 9090) — also serves /health
EXPOSE 9090

WORKDIR /data

# Health check — polls the /health endpoint on the metrics port
# Adjust --metrics-addr in CMD to match if overridden
HEALTHCHECK --interval=15s --timeout=5s --start-period=40s --retries=3 \
    CMD curl -sf http://localhost:9090/health || exit 1

# Default command
ENTRYPOINT ["/usr/local/bin/coinject"]
CMD ["--help"]
