# ── Stage 1: build ──────────────────────────────────────────────────────────
FROM rust:1.99.1-slim@sha256:4df2ae8cfdfebdfc13a65da7f253b73d68983dc0339f2719bc17a279c913b13d AS builder

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
FROM gcr.io/distroless/cc-debian13@sha256:159783207c2cd44c2aa5715961d13c8612368ac9bd450f887e3f08fc8ea461e3

WORKDIR /

COPY --from=builder /build/target/release/ha-proxy /usr/local/bin/ha-proxy

EXPOSE 8080

# nonroot/65532 - https://github.com/GoogleContainerTools/distroless/blob/main/common/variables.bzl
USER 65532
ENTRYPOINT ["/usr/local/bin/ha-proxy"]
