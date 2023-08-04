FROM rust:1.70-bullseye AS builder
RUN rustup target add wasm32-unknown-unknown
RUN cargo install -f trunk

# This is failing right now due to GLIBC.
# RUN wget -qO- https://github.com/thedodd/trunk/releases/download/v0.17.3/trunk-x86_64-unknown-linux-gnu.tar.gz | tar -xzf-
# RUN chmod +x trunk
# RUN cp trunk /usr/local/bin

WORKDIR /app/build
COPY . .
RUN cargo build -p cli
RUN cargo build -p plugin-example-shared
RUN cp .env.prod .env
RUN cd yew && trunk build

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y libsqlite3-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/build/target/debug/cli /app
COPY --from=builder /app/build/yew/dist /app/assets
RUN ls -alhR /app

EXPOSE 3000

CMD [ "/app/cli" ]
