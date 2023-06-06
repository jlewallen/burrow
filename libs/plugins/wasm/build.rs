use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=../examples/wasm/src");
    println!("cargo:rerun-if-changed=../rpc/src");
    println!("cargo:rerun-if-changed=../core/src");
    println!("cargo:rerun-if-changed=../../engine");
    println!("cargo:rerun-if-changed=../../kernel");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = std::path::Path::new(&out_dir).join("scripts_target");
    let status = Command::new("cargo")
        .args(["build"])
        .args(["--profile", "release-wasm"])
        .args(["--package", "plugin-example-wasm"])
        .args(["--target", "wasm32-unknown-unknown"])
        .args(["--target-dir", dest_path.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());

    std::fs::copy(
        dest_path.join("wasm32-unknown-unknown/release-wasm/plugin_example_wasm.wasm"),
        std::path::Path::new("./assets").join("plugin_example_wasm.wasm"),
    )
    .unwrap();
}
