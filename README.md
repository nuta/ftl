# FTL

FTL is a hybrid kernel operating system aiming to be the drop-in third choice for *NIX-like environments, after Linux and BSD. It aims to be:

- **Secure:** Eliminate or mitigate vulnerabilities with minimal core, capabilities-based security, language-enforced safety (Rust), and more proactive security measures.
- **Updatable:** Update or patch the OS without rebooting. Per-container kernels and microkernel-like OS servers.
- **Lightweight:** Keep its footprint small to run even on constrained devices, and to develop and test quickly.

## Running locally

Install Rust toolchain, LLVM tools, and QEMU:

```
brew install rustup llvm qemu       # macOS
apt install rustup llvm qemu-system # Ubuntu
```

Build and run:

```
./run.sh
```

## Design

FTL is a hybrid kernel. It has a small core (like microkernel), but also includes thin multiplexing (like [exokernel](https://dl.acm.org/doi/10.1145/224057.224076)), and language-isolated OS components (like [TockOS](https://dl.acm.org/doi/10.1145/3131672.3136988), [RedLeaf](https://www.usenix.org/conference/osdi20/presentation/narayanan-vikram), and [framekernel](https://www.usenix.org/conference/atc25/presentation/peng-yuke)).

### Per-container kernels

FTL implements Linux ABI as one of OS interfaces (personality) on top of the kernel core, similar to FreeBSD's [Linuxulator](https://wiki.freebsd.org/Linuxulator), Windows Subsystem for Linux (WSL 1), and Linux's [personality (2)](https://man7.org/linux/man-pages/man2/personality.2.html)/[Wine](https://www.winehq.org)/[Darling](https://github.com/darlinghq/darling). Multiple personalities can coexist.

This lets you run Linux containers on their own isolated Linux-like kernels, similar to [gVisor](https://github.com/google/gvisor), but without the overhead of system call hooking. You can also update the Linux-like kernel simply by starting a new container, without rebooting the machine.

### Thin TCP/IP and file system multiplexing

Unlike microkernels where everything is implemented as user-space processes, FTL has a thin multiplexer of file system and TCP/IP inside the kernel. This is similar to Exokernel, but FTL does not aim to expose the raw hardware details.

This design allows per-container kernels to safely access shared resources such as TCP ports and files, and ultimately aims to improve resiliency against faulty device and file system drivers like [MINIX3](https://wiki.minix3.org/doku.php?id=www:documentation:reliability).

### Interceptors (planned)

Interceptor is a planned feature to control the behavior of OS components at runtime, just like middlewares in web frameworks. Rate limiting, security auditing, network packet routing, live patching, will be implemented as interceptors.

### Language-based in-kernel isolation

FTL kernel is written entirely in Rust to leverage its aliasing XOR mutability principle, good type system, and memory safety guarantees to make it robust against bugs and security vulnerabilities.

### Batteries included

FTL will be more similar to BSD than Linux. We plan to provide FTL as a minimalistic OS with userspace utilities integrated nicely. This will include at least: kernel, OS servers, init system, container management, and some basic utilities like shell.
