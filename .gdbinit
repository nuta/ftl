set confirm off
set history save on
set print pretty on
set print demangle
set print asm-demangle
set disassemble-next-line on
set architecture aarch64
file ftl.elf
target remote 127.0.0.1:7789
