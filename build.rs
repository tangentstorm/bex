//! build script for bex.
//! This generates a small rust file that lets bex
//! report what options were used for compilation.
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let opt_level = env::var_os("OPT_LEVEL").unwrap();
    let bex_version = env!("CARGO_PKG_VERSION");
    let dest_path = Path::new(&out_dir).join("bex-build-info.rs");
    fs::write(
        &dest_path,
        format!("
        const BEX_VERSION : &str = {bex_version:?};
        const BEX_OPT_LEVEL : &str = {opt_level:?};
        ")
    ).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}
