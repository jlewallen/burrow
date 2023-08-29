FROM rust:1.71-bullseye AS base
WORKDIR /app

FROM base AS tooling
RUN cargo install -f cargo-chef && cargo install -f sccache && cargo install -f trunk

FROM tooling AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM tooling AS builder
ENV SCCACHE_CACHE_SIZE="5G"
ENV SCCACHE_DIR=/cache/sccache
RUN rustup target add wasm32-unknown-unknown
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/cache/sccache cargo chef cook -p plugin-example-shared -p cli --recipe-path recipe.json
RUN --mount=type=cache,target=/cache/sccache cargo chef cook -p plugin-example-shared -p cli --release --recipe-path recipe.json
COPY . .
RUN --mount=type=cache,target=/cache/sccache cargo build --release -p cli && sccache --show-stats
RUN --mount=type=cache,target=/cache/sccache cargo build --release -p plugin-example-shared && sccache --show-stats

RUN cp .env.prod .env
WORKDIR /app/web
RUN --mount=type=cache,target=/cache/sccache trunk build

WORKDIR /app
RUN ls -alh

FROM builder AS tests
RUN --mount=type=cache,target=/cache/sccache cargo build --tests --workspace
RUN --mount=type=cache,target=/cache/sccache cargo build --benches --workspace

FROM base
RUN apt-get update && apt-get install -y libsqlite3-dev && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/*.so /app
COPY --from=builder /app/target/release/cli /app
COPY --from=builder /app/web/dist /app/assets
RUN ls -alhR /app

EXPOSE 3000

CMD [ "/app/cli" ]
