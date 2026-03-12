#!/bin/sh
set -eu
if [ "${1:-}" = "--self-test" ]; then
  exit 0
fi
printf "example-compositor:%s\n" "${MACLAND_MODE:-unset}" > "${MACLAND_OUTPUT_FILE:-/tmp/macland-example-compositor.out}"
exit 0

