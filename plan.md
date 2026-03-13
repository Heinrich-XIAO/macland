# Plan: `macland` Native macOS Host for Running Wayland Compositors

## Execution Status
- `[PARTIAL]` Summary
- `[DONE]` Stack
- `[PARTIAL]` Runtime Model
- `[PARTIAL]` Fullscreen Session Model
- `[DONE]` Public Interfaces
- `[PARTIAL]` Compatibility Strategy
- `[DONE]` Build And Toolchain Plan
- `[PARTIAL]` Test Strategy
- `[PARTIAL]` Implementation Sequence
- `[PARTIAL]` Test Cases And Acceptance
- `[DONE]` Assumptions And Defaults

## Summary
Build a greenfield native macOS platform that lets a user select a Wayland compositor repo, build it on macOS, run its upstream tests where possible, and launch it inside a dedicated fullscreen macOS host session.

The design is optimized for low RAM and low latency:
- no VM
- no Electron/WebView/Chromium stack
- no web UI in the hot path
- Metal for accelerated rendering
- a thin native host plus one compositor child process

The plan targets Apple Silicon on recent macOS first, uses a CLI-first workflow, requires macOS permissions when needed for strong session control, and defers XWayland to a later phase.

## Stack
Chosen stack:
- `Swift + AppKit + MetalKit` for the macOS app shell, fullscreen window/session behavior, permissions UX, and host-side display/input integration
- `Rust` for the core runtime, repo orchestration, adapter resolution, test execution, artifact capture, and most compatibility-layer logic
- `C / Objective-C++` only for narrow low-level shims where Rust must interoperate with macOS APIs or Linux-oriented native interfaces

Explicit non-goals for the stack:
- no Node/TypeScript runtime in production paths
- no browser-based UI
- no pure-Swift low-level portability layer
- no single-process embedding requirement for arbitrary repos

## Runtime Model
Use a thin split-process architecture:
1. `macland-host`
A small Swift app that owns the fullscreen macOS window/session and presents compositor output.

2. `macland-core`
A Rust control plane launched by the host or CLI. It resolves adapters, starts the compositor binary, manages test execution, and brokers runtime state.

3. `compositor process`
The upstream-built compositor executable runs as a child process. This preserves the “pick a repo, build it, run it” goal without forcing per-repo embedding work.

Reason for this choice:
- arbitrary upstream repos usually produce executables, not embeddable libraries
- split-process preserves repo-agnostic support
- RAM cost is modest if the host stays thin
- crash isolation remains acceptable without turning the system into a VM

## Fullscreen Session Model
- Default mode is one borderless fullscreen host on one display
- The host attempts maximal takeover allowed by macOS
- Require Accessibility/Input Monitoring permissions if needed
- Hide standard chrome and suppress normal UI where macOS permits
- Windowed mode exists only for engineering/debug builds
- Multi-display support is deferred until the single-display backend is stable

## Public Interfaces
Primary CLI:
- `macland doctor`
- `macland repo add <git-url> [--rev <commit>]`
- `macland repo sync <repo-id>`
- `macland inspect <repo-id>`
- `macland build <repo-id>`
- `macland test <repo-id> [--upstream] [--conformance]`
- `macland run <repo-id> [--fullscreen|--windowed-debug]`

Adapter manifest: `macland.toml`
Required fields:
- `id`
- `repo`
- `rev`
- `build_system`
- `configure`
- `build`
- `test`
- `entrypoint`
- `env`
- `sdk_features`
- `protocol_expectations`
- `patch_policy`

Support states returned by `inspect`:
- `buildable`
- `upstream_tests_pass`
- `conformance_pass`
- `fullscreen_run_pass`
- `tier`

## Compatibility Strategy
Primary approach:
- provide a macOS backend SDK plus build/runtime shims
- keep upstream source changes at zero by default
- allow a small compatibility patch lane only when unavoidable

Initial target order:
1. wlroots-family compositors
2. libweston/Weston-style compositors
3. other custom compositor stacks

Platform assumptions:
- Metal is the only accelerated rendering path
- a software-rendering fallback exists for tests and unsupported acceleration cases
- XWayland is phase 2+, not phase 1 acceptance

## Build And Toolchain Plan
- Root project uses SwiftPM for host/CLI packaging plus native Rust crates
- A workspace-managed tool bootstrap installs or vendors missing dependencies such as Meson, Ninja, `pkg-config`, Wayland protocol tools, and shared native libraries
- Users should not need to hand-assemble most toolchain pieces
- Build adapters support `meson`, `cmake`, `cargo`, `autotools`, `make`, and `custom`

## Test Strategy
Testing is a first-class requirement.

1. Upstream test reuse
- Meson repos: run `meson test`
- CMake repos: run `ctest`
- Cargo repos: run `cargo test`
- custom repos: adapter-declared test commands

2. `macland` conformance suite
- launch compositor inside the host session
- connect reference Wayland clients
- verify first frame, input delivery, focus, surface lifecycle, exit/restart behavior, and fullscreen session rules

3. Host/platform tests
- Swift/XCTest for fullscreen host lifecycle, permission flows, and display/input integration
- Rust integration tests for adapters, orchestration, logging, and artifact handling

Support policy:
- compositors are supported by tier
- full upstream pass is preferred but not mandatory
- known platform-inapplicable failures can be waived if conformance and runtime criteria pass

## Implementation Sequence
1. `[DONE]` Build `macland doctor` and workspace-local tool bootstrap
2. `[DONE]` Implement `macland-host` fullscreen app shell in Swift/AppKit/MetalKit
3. `[DONE]` Implement Rust `macland-core` CLI/orchestrator
4. `[DONE]` Add thin host-to-core control boundary and compositor child-process launcher
5. `[PARTIAL]` Implement macOS display/input/session backend shims
6. `[DONE]` Support Meson/CMake/Cargo adapters first
7. `[DONE]` Add upstream test execution and normalized result reporting
8. `[PARTIAL]` Add conformance harness with Wayland reference clients
9. `[PARTIAL]` Land first wlroots-family compositor integration
10. `[PARTIAL]` Expand to Weston/libweston and then broader compositor families
11. `[TODO]` Add XWayland only after native Wayland compositor support is stable

## Test Cases And Acceptance
- `[DONE]` `doctor` reports missing tools, permissions, and SDK pieces clearly
- `[DONE]` a Meson compositor repo can be cloned, built, tested, and launched
- `[DONE]` a CMake compositor repo can do the same
- `[DONE]` a Cargo compositor repo can do the same
- `[TODO]` first frame appears inside the fullscreen host
- `[TODO]` pointer and keyboard events reach clients correctly
- `[TODO]` focus behavior remains correct under fullscreen takeover
- `[PARTIAL]` compositor crash is detected and reported without corrupting host state
- `[PARTIAL]` windowed debug mode matches fullscreen behavior except presentation
- `[DONE]` support tier output is deterministic and reproducible across reruns

## Assumptions And Defaults
- repo is currently empty, so implementation starts from scratch
- Apple Silicon + recent macOS is the initial baseline
- low RAM and good performance take precedence over maximal abstraction
- CLI-first is the primary operator interface
- fullscreen host is the default product mode
- debug windowed mode is allowed but not user-facing by default
- no VM and no browser runtime are allowed
- thin split-process is required to preserve generic repo support
- upstream patching is a last resort, not the normal path
