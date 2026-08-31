#!/bin/bash
set -eu

./build.sh
bun test tests
