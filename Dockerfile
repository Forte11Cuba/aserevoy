# The bot: compiled in a Rust image, run in a small Debian one.
# Migrations and data/zones.geojson are embedded at compile time;
# data/streets.sqlite is read at run time.

FROM rust:1.94-slim-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
# Dependencies first, so code changes don't rebuild them.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src
COPY src src
COPY migrations migrations
COPY data data
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home-dir /var/lib/aserevoy aserevoy \
    && mkdir -p /var/lib/aserevoy && chown aserevoy /var/lib/aserevoy
WORKDIR /app
COPY --from=build /app/target/release/aserevoy /usr/local/bin/aserevoy
COPY data/streets.sqlite data/streets.sqlite
USER aserevoy
# The database lives in a volume (see docker-compose.yml).
ENV DATABASE_URL=sqlite:///var/lib/aserevoy/aserevoy.db \
    STREETS_DB=/app/data/streets.sqlite
CMD ["aserevoy"]
