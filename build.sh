#!/bin/bash
set -eu

RELEASE=${RELEASE:-}
ARCH=${ARCH:-x64}

export CARGO_TERM_HYPERLINKS=false

CARGOFLAGS=(
    -Z build-std=core,alloc
    -Z build-std-features=compiler-builtins-mem
    -Z json-target-spec
)

if [[ -n "${RELEASE:-}" ]]; then
    CARGOFLAGS+=(--release)
    target="release"
else
    target="debug"
fi

cargo build "${CARGOFLAGS[@]}" --target libs/rust/ftl_api/src/arch/$ARCH/server.json \
  --manifest-path servers/hello/Cargo.toml
cargo build "${CARGOFLAGS[@]}" --target kernel/src/arch/$ARCH/kernel.json \
  --manifest-path kernel/Cargo.toml

cp target/kernel/$target/kernel ftl.elf
