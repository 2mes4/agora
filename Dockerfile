# syntax=docker/dockerfile:1
# Multi-stage build: compile the workspace in a full toolchain image,
# ship only the release binaries in a slim runtime.

FROM rust:1-bookworm AS builder
WORKDIR /build

# Dependency layer first (invalidated only when manifests change).
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY examples ./examples
RUN cargo build --release -p agora-server -p direct-delegate

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/agora-server /usr/local/bin/agora-server
COPY --from=builder /build/target/release/direct-delegate /usr/local/bin/agora-demo

# Gateway (A2A + registry + context APIs). The SDK demo (agora-demo) is also
# available for experimentation: docker run --rm 2mes4/agora /usr/local/bin/agora-demo
EXPOSE 7100

ENV AGORA_BIND=0.0.0.0:7100

ENTRYPOINT ["/usr/local/bin/agora-server"]
CMD ["--demo-agent"]
