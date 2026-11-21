# Changelog

## 0.3.0 (Nov 21, 2026)

- linux: Renamed from LX.
- linux: Dynamically linked executables support.
- linux: file-backed `mmap(2)`.
- linux: Environment variable support.
- linux: Better signal handling (`SA_SIGINFO`, `sigaltstack(2)`, `rt_sigprocmask(2)`, `tgkill(2)`).
- linux: `openat(2)`, `stat(2)`, `vfork(2)`, `pselect6(2)`, `ioctl(2)`, `readv(2)`, `pread64(2)`, `uname(2)`, `getsockopt(2)`, `getsockname(2)`, `getpeername(2)`, and more.
- Firecracker support: kvmclock, local APIC timer, modern virtio-mmio, and static IP configuration from the boot cmdline.
- kernel: Use radix tree for large VMOs (larger virtual address spaces).

## 0.2.0 (Oct 28, 2026)

- LX: Zone, our Linux container/FreeBSD jail like isolated environment.
- LX: Virtual file system system calls (e.g. `getdents64(2)`) and better epoll.
- Virtual NIC API for more flexible networking.
- kernel: Userspace page fault handling (`userfaultfd(2)`-like feature).
- kernel: CPU time and user memory quota.

## 0.1.0 (Oct 3, 2026)

- LX: threads (`clone(2)`), futex, epoll, signals, tty, brk, mmap dup3, pipe, eventfdm and more.
- Console system calls (serial ports) to allow shell access.
- Wall-clock time API.
- virtio-mmio and [QEMU microVM](https://www.qemu.org/docs/master/system/i386/microvm.html) support.
- Anonymous memory pages are now allocated lazily.
- Blocking Linux system calls are now interruptible by signals.
- x86-64 SMEP/SMAP support.

## 0.0.1 (Sep 15, 2026)

The first release. Hello world!
