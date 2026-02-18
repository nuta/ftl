#!/bin/bash
set -eu

function pad-to-4kb() {
    local file=$1
    local size=$(stat -f %z $file)
    local pad=$(( (4096 - (size % 4096)) % 4096 ))
    truncate -s $(( size + pad )) $file
}

/opt/homebrew/opt/llvm/bin/clang \
  -O2 -target x86_64-unknown-none-elf \
  -nostdlib -ffreestanding \
  -c -o syscall.o syscall.S
/opt/homebrew/opt/lld/bin/ld.lld -T syscall.ld -o syscall.elf syscall.o
llvm-objcopy -O binary syscall.elf syscall.bin
pad-to-4kb syscall.bin
