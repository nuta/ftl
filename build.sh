#!/bin/bash
set -eu

APPS=(httpd)
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
mkdir -p initfs

# Build apps.
mkdir -p initfs/bin
zig cc -std=c23 -Os -target x86_64-linux-musl -static -no-pie \
    -ffunction-sections -fdata-sections -Wl,--gc-sections \
    -DINDEX_HTML_LENGTH="$(wc -c < apps/httpd/index.html | xargs)" \
    -DNOT_FOUND_HTML_LENGTH="$(wc -c < apps/httpd/404.html | xargs)" \
    apps/httpd/main.c -o initfs/bin/httpd
printf 'bin/httpd\0' >> initfs.list

# Build initfs.
pushd initfs
cpio -o -H newc -0 < ../initfs.list > ../initfs.cpio
popd

# Build userspace OS.
FTL_LOG_PREFIX="[$(printf '%-10s' "lx")] " \
    cargo build "${CARGOFLAGS[@]}" --target libs/ftl/src/arch/$ARCH/user.json \
       --manifest-path lx/Cargo.toml
cp target/user/$target/lx lx.elf

# Build kernel.
FTL_LOG_PREFIX="[$(printf '%-10s' "kernel")] " \
  cargo build "${CARGOFLAGS[@]}" --target kernel/src/arch/$ARCH/kernel.json \
    --manifest-path kernel/Cargo.toml
cp target/kernel/$target/kernel ftl.elf
