#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
log_dir="$repo_root/repos/sway/artifacts/run"
mkdir -p "$log_dir"
printf '%s Mod+Return pressed\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" >> "$log_dir/shortcut.log"

export MACLAND_WORKSPACE_ROOT="$repo_root"
bridge_bin="$repo_root/target/release/macland-macos-bridge"
if [ ! -x "$bridge_bin" ]; then
  bridge_bin="$repo_root/target/debug/macland-macos-bridge"
fi
if [ ! -x "$bridge_bin" ]; then
  printf '%s macland-macos-bridge missing; run `cargo build -p macland-macos-bridge`\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" >> "$log_dir/shortcut.log"
  exit 1
fi

exec "$bridge_bin" --bundle-id com.apple.Terminal --title "Terminal"
