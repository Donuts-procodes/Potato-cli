# ==============================================================================
# Potato CLI — Production Multi-Stage Dockerfile
# Native Rust Agent Engine with Node.js, Bun, Yarn, pnpm, and Git Toolchain
# ==============================================================================

# ------------------------------------------------------------------------------
# Stage 1: Build Native Rust Binary
# ------------------------------------------------------------------------------
FROM rust:bookworm AS builder

WORKDIR /build

# Copy dependency manifests first for build caching
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Compile release binary
RUN cargo build --release -p potato-cli --bin potato

# ------------------------------------------------------------------------------
# Stage 2: Runtime Image with Full Developer Toolchain
# ------------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

LABEL maintainer="Potato CLI Team" \
      description="Autonomous Super Loop Agent Engine with Multi-Runtime Toolchain"

ENV DEBIAN_FRONTEND=noninteractive \
    SHELL=/bin/bash \
    BUN_INSTALL=/root/.bun \
    PATH="/root/.bun/bin:/usr/local/bin:${PATH}"

# Install core system dependencies, Python, Git, OpenSSL, and curl
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    git \
    openssh-client \
    build-essential \
    python3 \
    python3-pip \
    python3-venv \
    unzip \
    && rm -rf /var/lib/apt/lists/*

# Install Node.js (v20 LTS), npm, yarn, and pnpm
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && npm install -g yarn pnpm \
    && rm -rf /var/lib/apt/lists/*

# Install Bun
RUN curl -fsSL https://bun.sh/install | bash \
    && ln -s /root/.bun/bin/bun /usr/local/bin/bun

# Copy native potato binary from builder stage
COPY --from=builder /build/target/release/potato /usr/local/bin/potato
RUN chmod +x /usr/local/bin/potato

# Verify all runtimes and tools are operational
RUN potato --version \
    && node --version \
    && npm --version \
    && yarn --version \
    && pnpm --version \
    && bun --version \
    && git --version \
    && python3 --version

# Setup workspace directory
WORKDIR /workspace

# Safe directory for git inside container
RUN git config --global --add safe.directory /workspace

# Default command
ENTRYPOINT ["potato"]
CMD ["--help"]
