#!/bin/sh
set -eu
test -x bin/example-compositor
./bin/example-compositor --self-test

