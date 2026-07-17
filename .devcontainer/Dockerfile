# Lattice cell / Dev Container image: Node 22 + pnpm + Rust (no Nix, no Tauri).
# Adapted from willmortimer/Templates `pnpm-monorepo` + `rust-cli` (e7fac162).

FROM mcr.microsoft.com/devcontainers/javascript-node:1-22-bookworm

ARG RUST_VERSION=1.85.0

USER root
RUN apt-get update \
  && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    curl \
    git \
    libssl-dev \
    pkg-config \
  && rm -rf /var/lib/apt/lists/* \
  && corepack enable

USER node
ENV CARGO_HOME=/home/node/.cargo \
    RUSTUP_HOME=/home/node/.rustup \
    PATH=/home/node/.cargo/bin:${PATH}

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain "${RUST_VERSION}" --profile minimal \
  && rustup component add rustfmt clippy
