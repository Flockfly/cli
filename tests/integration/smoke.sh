#!/bin/bash
set -euo pipefail

BINARY="${1:-target/release/flockfly}"

test -x "$BINARY"
"$BINARY" --version | grep -q "flockfly"
"$BINARY" --help | grep -q "publish"
"$BINARY" search --help | grep -q -- "--load"
"$BINARY" init | grep -q 'flockfly load skill_pxJxZr7CMBMk'
