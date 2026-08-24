fn main() {
    cxx_build::bridge("src/lib.rs")
        .file("src/sly.cpp")
        .include("include")
        .std("c++20")
        .compile("sly_cxx_bridge");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/sly.cpp");
    println!("cargo:rerun-if-changed=include/sly/sly.hpp");
}
