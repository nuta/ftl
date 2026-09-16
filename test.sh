#!/bin/bash
set -eu

if [[ ! -n "${SKIP_BUILD:-}" ]]; then
  ./build.sh
fi

bun test tests
