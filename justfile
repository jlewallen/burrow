export RUST_LOG := "info"

default: test

build:
    cargo build --workspace --all-targets

test: build
    cargo test --workspace

eval: build
    cargo run -- eval

shell: build
    cargo run -- shell

serve: build
    cargo run -- serve

look: build
    cargo run -- eval --text look --text look --text look --separate-sessions

clean:
    rm -rf target
