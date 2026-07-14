#!/bin/bash
set -eu

APPS=(hello)
SERVERS=(lx)
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

echo -n > initfs.list
mkdir -p initfs/servers

# Build apps.
mkdir -p initfs/bin
zig cc -O2 -target x86_64-linux-musl apps/hello/hello.S -ffreestanding -nostdlib -o initfs/bin/hello

# Build servers.
for server in "${SERVERS[@]}"; do
  FTL_LOG_PREFIX="[$(printf '%-10s' "$server")] " \
    cargo build "${CARGOFLAGS[@]}" --target libs/rust/ftl_api/src/arch/$ARCH/server.json \
      --manifest-path servers/$server/Cargo.toml

  cp target/server/$target/lib$server.so initfs/servers/$server.elf
  printf 'servers/%s.elf\0' "$server" >> initfs.list
done

# Build initfs.
pushd initfs
cpio -o -H newc -0 < ../initfs.list > ../initfs.cpio
popd

# Build kernel.
FTL_LOG_PREFIX="[$(printf '%-10s' "kernel")] " \
  cargo build "${CARGOFLAGS[@]}" --target kernel/src/arch/$ARCH/kernel.json \
    --manifest-path kernel/Cargo.toml
cp target/kernel/$target/kernel ftl.elf
