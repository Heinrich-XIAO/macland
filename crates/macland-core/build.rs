fn main() {
    cc::Build::new()
        .file("native/src/macland_sdk.c")
        .include("native/include")
        .compile("macland_sdk");

    println!("cargo:rerun-if-changed=native/include/macland_sdk.h");
    println!("cargo:rerun-if-changed=native/src/macland_sdk.c");
}
