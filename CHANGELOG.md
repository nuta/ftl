# Changelog

## 0.0.1 (Sep 15, 2026)

The first release. Hello world!

## 0.1.0 (Oct 3, 2026)

- LX: threads (`clone(2)`), futex, epoll, signals, tty, brk, mmap dup3, pipe, eventfdm and more.
- Console system calls (serial ports) to allow shell access.
- Wall-clock time API.
- virtio-mmio and [QEMU microVM](https://www.qemu.org/docs/master/system/i386/microvm.html) support.
- Anonymous memory pages are now allocated lazily.
- Blocking Linux system calls are not interruptible by signals.
- x86-64 SMEP/SMAP support.
