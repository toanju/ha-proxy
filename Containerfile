# ── Stage 1: build ──────────────────────────────────────────────────────────
FROM rust:1.97.1-slim@sha256:754a8924e308fb20a327febeda1a07053a2b0fd7474b5ac1cc460a6d33ab18f3 AS builder

WORKDIR /build

# Copy manifest files first so that dependency compilation is cached in a
# separate layer and only re-run when Cargo.toml / Cargo.lock change.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && echo 'fn main(){}' > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Build the real binary. Touch src/main.rs to ensure cargo re-compiles it
# even though the modification time of the copied file may match the cache.
COPY src ./src
RUN touch src/main.rs \
    && cargo build --release

# ── Stage 2: runtime ────────────────────────────────────────────────────────
FROM gcr.io/distroless/cc-debian13@sha256:ed7c407fd64eb0af9dddb9456b94cee188a40a7f53cf38c9836e1e9ae14fca02

WORKDIR /

COPY --from=builder /build/target/release/ha-proxy /usr/local/bin/ha-proxy

EXPOSE 8080

# nonroot/65532 - https://github.com/GoogleContainerTools/distroless/blob/main/common/variables.bzl
USER 65532
ENTRYPOINT ["/usr/local/bin/ha-proxy"]
