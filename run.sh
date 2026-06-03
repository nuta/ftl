#!/bin/bash
set -eu

./build.sh

set +e
qemu-system-x86_64 \
  -m 128 -cpu qemu64,+fsgsbase -kernel ftl.elf \
  -initrd initfs.cpio \
  -nographic -serial mon:stdio --no-reboot -gdb tcp::7778 \
  -d cpu_reset,unimp,guest_errors,int -D qemu.log \
  -device isa-debug-exit,iobase=0x501,iosize=0x04 \
  -netdev user,id=net0,hostfwd=tcp:127.0.0.1:30080-:80 \
  -device virtio-net-pci,netdev=net0 \
  -object filter-dump,id=filter0,netdev=net0,file=network.pcap
