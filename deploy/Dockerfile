# syntax=docker/dockerfile:1.7
FROM rust:1-bookworm AS build
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release -p termoso-server \
    && cp target/release/termoso-server /termoso-server

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /termoso-server /usr/local/bin/termoso-server
ENV TERMOSO_BIND=0.0.0.0:8080
EXPOSE 8080
USER nonroot
ENTRYPOINT ["/usr/local/bin/termoso-server"]
