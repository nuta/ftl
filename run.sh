#!/bin/bash
set -eu

ARCH=x64
APPS=(virtio_net)

cargo build \
  -Z build-std=core,alloc \
  -Z build-std-features=compiler-builtins-mem \
  --manifest-path kernel/Cargo.toml \
  --target kernel/src/arch/$ARCH/kernel.json

for app in "${APPS[@]}"; do
  cargo build \
    -Z build-std=core,alloc \
    -Z build-std-features=compiler-builtins-mem \
    --manifest-path apps/$app/Cargo.toml \
    --target libs/rust/ftl/src/arch/$ARCH/user.json
done

cp target/kernel/debug/kernel ftl.elf

bun mkinitfs.ts

if [[ ${BUILD_ONLY:-} != "" ]]; then
  exit 0
fi

set +e
qemu-system-x86_64 \
  -m 128 -cpu qemu64,+fsgsbase -kernel ftl.elf -initrd initfs.tar \
  -nographic -serial mon:stdio --no-reboot -gdb tcp::7778 \
  -d cpu_reset,unimp,guest_errors,int -D qemu.log \
  -netdev user,id=net0 \
  -device virtio-net-pci,netdev=net0 \
  -object filter-dump,id=filter0,netdev=net0,file=network.pcap
