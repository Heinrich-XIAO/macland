# `macland.toml`

Each compositor repo can provide an adapter manifest named `macland.toml`.

## Required keys

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

## Example

```toml
id = "labwc"
repo = "https://github.com/labwc/labwc.git"
rev = "main"
build_system = "meson"
configure = ["meson", "setup", "build"]
build = ["meson", "compile", "-C", "build"]
test = ["meson", "test", "-C", "build"]
entrypoint = ["./build/labwc"]
patch_policy = "prefer-none"
sdk_features = ["metal-fast-path", "seat-v1"]
protocol_expectations = ["xdg-shell", "layer-shell"]

[env]
MACLAND_MODE = "1"
```
