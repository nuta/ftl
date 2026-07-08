# FTL

FTL is a hybrid kernel operating system aiming to be the drop-in third choice for *NIX-like environments, after Linux and BSD. It aims to be:

- **Secure:** Eliminate or mitigate vulnerabilities with minimal core, capabilities-based security, language-enforced safety (Rust), and more proactive security measures.
- **Updatable:** Per-container kernels and microkernel-like OS servers enable OS updates/patching without rebooting.
- **Lightweight:** Keep its footprint small to run even on constrained devices, and to develop and test the OS quickly.

## Running locally

Install Rust toolchain, LLVM tools, and QEMU:

```
brew install rustup llvm qemu        # macOS
apt  install rustup llvm qemu-system # Ubuntu
```

Build and run:

```
./run.sh
```

## Design

FTL is a hybrid kernel. It has a small core (like microkernel), but also includes thin multiplexing (like [exokernel](https://dl.acm.org/doi/10.1145/224057.224076)), and language-isolated OS components (like [TockOS](https://dl.acm.org/doi/10.1145/3131672.3136988), [RedLeaf](https://www.usenix.org/conference/osdi20/presentation/narayanan-vikram), and [framekernel](https://www.usenix.org/conference/atc25/presentation/peng-yuke)).

### Per-container kernels

FTL implements Linux ABI as an isolated OS component. This lets you run Linux containers on their own isolated Linux-like kernels, similar to [gVisor](https://github.com/google/gvisor), but without the overhead of system call hooking.

You can also update the Linux-like kernel simply by starting a new container, without rebooting the machine.

Foreign binary support (ABI emulation) is a well-established technique that can be seen in modern operating systems. Windows Subsystem for Linux (WSL 1), FreeBSD's [Linuxulator](https://wiki.freebsd.org/Linuxulator), and old Linux's [personality (2)](https://man7.org/linux/man-pages/man2/personality.2.html), for example. The key difference in FTL is that each container has its own isolated Linux-like kernel, implemented on top of a minimum core.

### Secure kernel without compromise

Similar to microkernels, most of OS services such as device drivers, file systems, and network stacks, and Linux compatibility layer are implemented as isolated OS services (servers) on top of the kernel.

Servers coexist in the kernel space with language-based isolation, relying on Rust's safety guarantees and sound and securely designed API for OS services. Language-based isolation is weaker than hardware-based ones used in traditional microkernels, but it enables _good-enough_ security without sacrificing performance.

### Interceptors (planned)

Interceptor is a planned feature to control the behavior of OS components at runtime, just like middlewares in web frameworks. Rate limiting, security auditing, network packet routing, live patching, will be implemented as interceptors.

### Batteries included

FTL will be more similar to BSD than Linux. We plan to provide FTL as a minimalistic OS with userspace utilities integrated nicely. This will include at least: kernel, OS servers, init system, container management, cloud platform integration, and some basic utilities like shell.

## License

FTL is dual-licensed under the MIT and Apache 2.0 licenses.
