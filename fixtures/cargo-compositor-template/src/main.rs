fn main() {
    let mode = std::env::var("MACLAND_MODE").unwrap_or_else(|_| "unset".to_string());
    println!("cargo-compositor:{mode}");
}

