# syntax=docker/dockerfile:1
FROM rust:1-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY fixtures/benchmark/relevance.json ./fixtures/benchmark/relevance.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --locked --release --package rekora-cli && cp target/release/rekora /build/rekora

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates git curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 rekora && useradd --uid 10001 --gid rekora --create-home rekora \
    && mkdir /workspace && chown rekora:rekora /workspace
COPY --from=build /build/rekora /usr/local/bin/rekora
COPY LICENSE /usr/share/doc/rekora/LICENSE
USER rekora
WORKDIR /workspace
EXPOSE 8080
ENTRYPOINT ["rekora"]
CMD ["mcp", "serve", "--stdio"]
