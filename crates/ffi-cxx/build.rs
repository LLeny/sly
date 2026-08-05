fn main() {
    cxx_build::bridge("src/lib.rs")
        .std("c++17")
        .compile("sly_cxx_bridge");
    println!("cargo:rerun-if-changed=src/lib.rs");
}
