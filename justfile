default_level := "info"

default:
    cargo test --all
    cargo build --all

eval: dynamics
    RUST_LOG={{ default_level }} cargo run -- eval

shell: dynamics
    RUST_LOG={{ default_level }} cargo run -- shell

serve: dynamics
    RUST_LOG={{ default_level }} cargo run -- serve

dynamics:
    cargo build --package plugins_example

look:
    RUST_LOG={{ default_level }} cargo run -- eval --text look --text look --text look

clean:
    rm -rf target
