export RUST_LOG := "info"

default: test

build:
    cargo build

test:
    cargo test --workspace

plugins:
    cargo build --package plugin-example-shared

eval: plugins
    cargo run -- eval

shell: plugins
    cargo run -- shell

serve: plugins
    cargo run -- serve

look: plugins
    cargo run -- eval --text look --text look --text look --separate-sessions

clean:
    rm -rf target
