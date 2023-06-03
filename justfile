default_level := "info"

default:
    cargo test --all
    cargo build --all

test:
    RUST_LOG={{ default_level }} cargo test --all

eval: external
    RUST_LOG={{ default_level }} cargo run -- eval

shell: external
    RUST_LOG={{ default_level }} cargo run -- shell

serve: external
    RUST_LOG={{ default_level }} cargo run -- serve

external:
    cargo build --package plugin-example-shared
    cargo build --package plugin-example-rpc

look:
    RUST_LOG={{ default_level }} cargo run -- eval --text look --text look --text look --separate-sessions

clean:
    rm -rf target
