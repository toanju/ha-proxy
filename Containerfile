# ── Stage 1: build ──────────────────────────────────────────────────────────
FROM rust:1.99.1-slim@sha256:b8e062b1c3a80183df83d9ba187cadd2b5ab84bc78d453dbfeb2fccd5f982247 AS builder

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
FROM gcr.io/distroless/cc-debian13@sha256:d74d144baa124dd9defd01d71872bb56c49d4441dba6153ccf790364f97e64b1

WORKDIR /

COPY --from=builder /build/target/release/ha-proxy /usr/local/bin/ha-proxy

EXPOSE 8080

# nonroot/65532 - https://github.com/GoogleContainerTools/distroless/blob/main/common/variables.bzl
USER 65532
ENTRYPOINT ["/usr/local/bin/ha-proxy"]
