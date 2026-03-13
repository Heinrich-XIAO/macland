# macland

`macland` is a native macOS host for building and running Wayland compositors inside a dedicated fullscreen session.

This repository currently contains:

- a Swift host shell for fullscreen or debug-windowed presentation
- a Rust control plane and CLI for repo management, build/test/run orchestration, and diagnostics
- a backend model for macOS session/output/input capabilities and mock runtime testing
- a shared `macland.toml` adapter format
- unit tests for the Rust orchestration layer and Swift host configuration

## Workspace layout

- `Package.swift`: Swift package for the host app and host support code
- `Cargo.toml`: Rust workspace for `macland-core`, the `macland-exec` control-plane binary, and support crates
- `Sources/`: Swift sources
- `Tests/`: Swift tests
- `crates/`: Rust crates
- `docs/`: design and adapter documentation

## Build

```bash
swift build
cargo test
```

## Run

```bash
./macland doctor
swift run macland-host --windowed-debug
```
