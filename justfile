export RUST_LOG := "info"

default: test build plugins

build:
    cargo build --workspace

test:
    cargo test --workspace

plugins:
    cargo build --package plugin-example-shared

deliver:
    cargo run -- eval --deliver

eval:
    cargo run -- eval

migrate:
    cargo run -- migrate

dump:
    cargo run -- dump

shell:
    cargo run -- shell

serve:
    cargo run -- serve

look *args='':
    cargo run -- eval --text look --text look --text look --separate-sessions {{args}}

bench: plugins
    cargo bench --workspace
    (cd libs/tests && cargo bench --bench simple -- --profile-time=5)

clean:
    rm -rf target
