export RUST_LOG := "info"

default: test

build:
    cargo build

test:
    cargo test --workspace

plugins:
    cargo build --package plugin-example-shared

deliver: plugins
    cargo run -- eval --deliver

eval: plugins
    cargo run -- eval

shell: plugins
    cargo run -- shell

serve: plugins
    cargo run -- serve

look *args='': plugins
    cargo run -- eval --text look --text look --text look --separate-sessions {{args}}

bench: plugins
    cargo bench --workspace
    (cd libs/tests && cargo bench --bench look -- --profile-time=5)

clean:
    rm -rf target
