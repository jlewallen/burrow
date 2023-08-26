FROM rust:1.71-bullseye AS base
WORKDIR /app

FROM base AS chef
RUN cargo install -f cargo-chef 

FROM chef AS chef_and_trunk
RUN cargo install -f trunk
# This is failing right now due to GLIBC.
# RUN wget -qO- https://github.com/thedodd/trunk/releases/download/v0.17.3/trunk-x86_64-unknown-linux-gnu.tar.gz | tar -xzf-
# RUN chmod +x trunk
# RUN cp trunk /usr/local/bin

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef_and_trunk AS builder
RUN rustup target add wasm32-unknown-unknown
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json
COPY . .
RUN cargo build --release -p cli
RUN cargo build --release -p plugin-example-shared
RUN cp .env.prod .env
RUN cd web && trunk build

FROM base
RUN apt-get update && apt-get install -y libsqlite3-dev && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/*.so /app
COPY --from=builder /app/target/release/cli /app
COPY --from=builder /app/web/dist /app/assets
RUN ls -alhR /app

EXPOSE 3000

CMD [ "/app/cli" ]
