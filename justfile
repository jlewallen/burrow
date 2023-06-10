export RUST_LOG := "info"

default: test

build:
    cargo build

test:
    cargo test --workspace

eval:
    cargo run -- eval

shell:
    cargo run -- shell

serve:
    cargo run -- serve

look:
    cargo run -- eval --text look --text look --text look --separate-sessions

clean:
    rm -rf target
