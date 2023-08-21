export RUST_LOG := "info"

default: test build plugins

setup:
    cp -n .env.default .env || true

build: setup
    cargo build --workspace

test: setup
    cargo test --workspace

testall: setup
    cargo test --workspace --no-fail-fast

plugins: setup
    cargo build --package plugin-example-shared

deliver: setup
    cargo run -- eval --deliver

eval *args='': setup
    cargo run -- eval {{args}}

migrate: setup
    cargo run -- migrate

dump: setup
    cargo run -- dump

shell: setup
    cargo run -- shell

serve: setup
    cargo run -- serve

look *args='': setup
    cargo run -- eval --text look --text look --text look --separate-sessions {{args}}

image:
    docker build -t jlewallen/burrow .

test-image:
    docker run --name test-burrow --rm -p 5000:3000 -v `pwd`:/app/data \
        -e RUST_LOG=debug,tower_http=debug \
        jlewallen/burrow \
        /app/cli serve --path /app/data/world.sqlite3

bench: plugins
    cargo bench --workspace
    (cd libs/tests && cargo bench --bench simple -- --profile-time=5)

clean:
    rm -rf target
