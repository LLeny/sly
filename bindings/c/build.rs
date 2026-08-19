use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());

    println!("cargo:rerun-if-changed=src/capi.rs");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");

    cbindgen::generate(&crate_dir)
        .expect("failed to generate the Sly C header")
        .write_to_file(crate_dir.join("include/sly.h"));
}
