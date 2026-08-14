# ── Build ─────────────────────────────────────────────────────────────────────
FROM rust:1-slim-bookworm AS builder

WORKDIR /build

# Native build dependencies for the crypto crates, plus git for the dependency
# on the protocol repository.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       build-essential cmake pkg-config libssl-dev git ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# `Cargo.lock` is committed, so this build resolves to exactly the versions that
# were tested rather than whatever is newest on the day it runs. That matters
# more than usual here: the build happens on deploy, not before it.
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked --bin di-relay

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/di-relay /usr/local/bin/di-relay

# libp2p: TCP and QUIC. Health and peer-id endpoint: $PORT, default 8080.
EXPOSE 4001/tcp
EXPOSE 4001/udp
EXPOSE 8080/tcp

# Run as a non-root user. The relay needs no privileged ports and holds no state
# on disk, so there is nothing it needs root for.
RUN useradd --system --no-create-home --shell /usr/sbin/nologin relay
USER relay

ENTRYPOINT ["/usr/local/bin/di-relay"]
