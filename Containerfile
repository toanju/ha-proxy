# ── Stage 1: build ──────────────────────────────────────────────────────────
FROM rust:1.100.0-slim@sha256:3b199fd6db7d709274df56565b8f4003f9e30d7bfcff9d53148c86053b7855e2 AS builder

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
FROM gcr.io/distroless/cc-debian13@sha256:dac90a25ffe03ebaace90cda886b081e61ab6cd77ac47eda51f7bcc15c320765

WORKDIR /

COPY --from=builder /build/target/release/ha-proxy /usr/local/bin/ha-proxy

EXPOSE 8080

# nonroot/65532 - https://github.com/GoogleContainerTools/distroless/blob/main/common/variables.bzl
USER 65532
ENTRYPOINT ["/usr/local/bin/ha-proxy"]
