# syntax=docker/dockerfile:1.7
#
# One image: the API server plus the built web cabinet (served on `/`).
#
#   server ─┐
#   wasm ───┴─> web ──> runtime (distroless)

ARG WASM_PACK_VERSION=0.15.0

# ── termoso-crypto → WebAssembly for the browser ─────────────────────────────
FROM rust:1-bookworm AS wasm
ARG WASM_PACK_VERSION
WORKDIR /src
RUN rustup target add wasm32-unknown-unknown
RUN set -eu; \
    arch="$(uname -m)"; \
    case "$arch" in \
      x86_64)  sum=c09f971ecaed9a2efc80fdcea7a00ef6b53c7fadc8c57d1f61b53a6aa66b668a ;; \
      aarch64) sum=e17ef0806381c3a0acb9c9ddad643a49facaa5a2ecf657a421d4d8f3357a24b7 ;; \
      *) echo "unsupported build arch: $arch" >&2; exit 1 ;; \
    esac; \
    name="wasm-pack-v${WASM_PACK_VERSION}-${arch}-unknown-linux-musl"; \
    curl -fsSLo /tmp/wp.tgz "https://github.com/wasm-bindgen/wasm-pack/releases/download/v${WASM_PACK_VERSION}/${name}.tar.gz"; \
    echo "${sum}  /tmp/wp.tgz" | sha256sum -c -; \
    tar -xzf /tmp/wp.tgz -C /tmp; \
    install -m 0755 "/tmp/${name}/wasm-pack" /usr/local/bin/wasm-pack; \
    rm -rf /tmp/wp.tgz "/tmp/${name}"
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps/desktop/src-tauri ./apps/desktop/src-tauri
RUN --mount=type=cache,id=cargo-registry-wasm,target=/usr/local/cargo/registry \
    --mount=type=cache,id=target-wasm,target=/src/target \
    wasm-pack build crates/termoso-wasm --release --target web \
      --out-dir /out/pkg --out-name termoso_wasm

# ── web cabinet (Vite) ───────────────────────────────────────────────────────
FROM node:25-bookworm-slim AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci --no-audit --no-fund
COPY web ./
COPY --from=wasm /out/pkg ./src/crypto/pkg
RUN npm run build

# ── API server ───────────────────────────────────────────────────────────────
FROM rust:1-bookworm AS server
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps/desktop/src-tauri ./apps/desktop/src-tauri
RUN --mount=type=cache,id=cargo-registry-server,target=/usr/local/cargo/registry \
    --mount=type=cache,id=target-server,target=/src/target \
    cargo build --release --locked -p termoso-server \
    && cp target/release/termoso-server /termoso-server

# ── runtime ──────────────────────────────────────────────────────────────────
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=server /termoso-server /usr/local/bin/termoso-server
COPY --from=web /web/dist /app/web
ENV TERMOSO_BIND=0.0.0.0:8080 \
    TERMOSO_WEB_DIR=/app/web
EXPOSE 8080
USER nonroot
ENTRYPOINT ["/usr/local/bin/termoso-server"]
