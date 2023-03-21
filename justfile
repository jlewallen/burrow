default_level := "debug"

default:
    cargo test --all

eval:
    RUST_LOG={{ default_level }} cargo run -- eval

shell:
    RUST_LOG={{ default_level }} cargo run -- shell

serve:
    RUST_LOG={{ default_level }} cargo run -- serve

clean:
    rm -rf target
