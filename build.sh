#!/bin/bash
set -eu

RELEASE=${RELEASE:-}
ARCH=${ARCH:-x64}

export CARGO_TERM_QUIET=true
export CARGO_TERM_HYPERLINKS=false

CARGOFLAGS=(
    -Z build-std=core,alloc
    -Z build-std-features=compiler-builtins-mem
    -Z json-target-spec
    --manifest-path kernel/Cargo.toml
    --target kernel/src/arch/$ARCH/kernel.json
)

if [[ -n "${RELEASE:-}" ]]; then
    CARGOFLAGS+=(--release)
    target="release"
else
    target="debug"
fi

zig cc -target x86_64-linux-gnu -nostdlib -ffreestanding \
  kernel/src/hello.S -o kernel/src/hello.elf
llvm-objcopy -O binary kernel/src/hello.elf kernel/src/hello.bin

cargo build "${CARGOFLAGS[@]}"
cp target/kernel/$target/kernel ftl.elf
