#!/bin/bash
set -eu

ARCH=x64
cargo build \
  -Z build-std=core \
  -Z build-std-features=compiler-builtins-mem \
  --manifest-path kernel/Cargo.toml \
  --target kernel/src/arch/$ARCH/kernel.json

cp target/kernel/debug/kernel ftl.elf

set +e
qemu-system-x86_64 \
  -m 128 -kernel ftl.elf \
  -nographic -serial mon:stdio --no-reboot \
  -d cpu_reset,unimp,guest_errors,int -D qemu.log \
  -gdb tcp::7778

# SeaBIOS prints an escape sequence which disables line wrapping, and messes up
# your terminal. Restore it.
printf '\033[?7h'
