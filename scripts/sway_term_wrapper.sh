#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
log_dir="$script_dir/../repos/sway/artifacts/run"
mkdir -p "$log_dir"
printf '%s Mod+Return pressed\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" >> "$log_dir/shortcut.log"

exec /usr/bin/open -a Terminal
