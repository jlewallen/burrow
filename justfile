default_level := "info"

default:
    # cargo build --package plugins_example
    cargo test --all
    cargo build --all

eval:
    RUST_LOG={{ default_level }} cargo run -- eval

shell:
    RUST_LOG={{ default_level }} cargo run -- shell

serve:
    RUST_LOG={{ default_level }} cargo run -- serve

clean:
    rm -rf target
