#!/bin/bash
set -eu

./build.sh
bun test --config tests/bunfig.toml tests
