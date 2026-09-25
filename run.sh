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

qemuflags=()
qemuflags+=(-machine microvm,acpi=off -m "${MEMORY:-128}")
qemuflags+=(-cpu qemu64,+fsgsbase,+xsave,+xsaveopt,+smep,+smap,+rdrand,+rdtscp)
qemuflags+=(-kernel ftl.elf -initrd lx.elf -append "$cmdline")
qemuflags+=(--no-reboot -gdb tcp::7778)
qemuflags+=(-d cpu_reset,unimp,guest_errors -D qemu.log)
qemuflags+=(-device isa-debug-exit,iobase=0x501,iosize=0x04)
qemuflags+=(-netdev user,id=net0,hostfwd=tcp:127.0.0.1:30080-:80)
qemuflags+=(-device virtio-net-device,netdev=net0)
qemuflags+=(-object filter-dump,id=filter0,netdev=net0,file=network.pcap)

if [[ -n "${GUI:-}" ]]; then
  qemuflags+=()
else
  qemuflags+=(-nographic -serial mon:stdio)
fi

set +e
qemu-system-x86_64 "${qemuflags[@]}"
