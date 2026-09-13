#!/bin/bash
set -eu

cmdline=""
if [[ $# -gt 0 ]]; then
  cmdline="ftl.lx.init=\"$1\""
else
  echo "run.sh: No init program specified, using default \"/bin/httpd\""
  cmdline="ftl.lx.init=\"/bin/httpd\""
fi

./build.sh

set +e
qemu-system-x86_64 \
  -machine pc,acpi=off -m 128 \
  -cpu qemu64,+fsgsbase,+xsave,+xsaveopt \
  -kernel ftl.elf -initrd lx.elf -append "$cmdline" \
  -nographic -serial mon:stdio --no-reboot -gdb tcp::7778 \
  -d cpu_reset,unimp,guest_errors,int -D qemu.log \
  -device isa-debug-exit,iobase=0x501,iosize=0x04 \
  -netdev user,id=net0,hostfwd=tcp:127.0.0.1:30080-:80 \
  -device virtio-net-pci,netdev=net0,romfile= \
  -object filter-dump,id=filter0,netdev=net0,file=network.pcap
