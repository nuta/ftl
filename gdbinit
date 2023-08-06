set confirm off
set history save on
set print pretty on
set disassemble-next-line auto
set architecture riscv:rv64
set riscv use-compressed-breakpoints yes
file ftl.elf
target remote 127.0.0.1:7777
