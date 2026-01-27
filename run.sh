#!/bin/bash
set -eu

ARCH=x64
cargo build \
  -Z build-std=core,alloc \
  -Z build-std-features=compiler-builtins-mem \
  --manifest-path kernel/Cargo.toml \
  --target kernel/src/arch/$ARCH/kernel.json

cargo build \
  -Z build-std=core,alloc \
  -Z build-std-features=compiler-builtins-mem \
  --manifest-path apps/hello/Cargo.toml \
  --target libs/rust/ftl/src/arch/$ARCH/user.json

cp target/kernel/debug/kernel ftl.elf

bun mkinitfs.ts

if [[ ${BUILD_ONLY:-} != "" ]]; then
  exit 0
fi

set +e
qemu-system-x86_64 \
  -m 128 -cpu qemu64,+fsgsbase -kernel ftl.elf -initrd initfs.tar \
  -nographic -serial mon:stdio --no-reboot \
  -d cpu_reset,unimp,guest_errors,int -D qemu.log \
  -gdb tcp::7778

# SeaBIOS prints an escape sequence which disables line wrapping, and messes up
# your terminal. Restore it.
printf '\033[?7h'
