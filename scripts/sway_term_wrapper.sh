#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
log_dir="$script_dir/../repos/sway/artifacts/run"
mkdir -p "$log_dir"
if [ "${MACLAND_TERM_AUTOSTART:-0}" != "1" ]; then
  printf '%s Mod+Return pressed\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" >> "$log_dir/shortcut.log"
fi

export MACLAND_TERM_LOG="$log_dir/terminal.log"
exec python3 "$script_dir/wayland_terminal.py" 2>>"$log_dir/terminal.err.log"
