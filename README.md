# FTL

FTL is a hybrid kernel operating system aiming to be the drop-in third choice for *NIX-like environments, after Linux and BSDs. It aims to be:

- **Secure:** Microkernel based on language-based isolation, capabilities-based security, per-container application kernels, and more proactive security measures.
- **Ergonomic:** Programmable and observable with interceptors (planned), easy-to-understand and testable codebase, handy end-to-end testing with TypeScript (planned), and quick edit-compile-run cycle.
- **Lightweight:** Keep its footprint small to run even on constrained devices, and develop the OS quickly.

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

## Roadmap

> :warning: This project is currently in pre-alpha stage.

- [ ] **Milestone: Make shell work (work-in-progress)**
  - [x] Kernel: thread and memory management
  - [x] Language-based server isolation
  - [x] System call emulation: Hello World from Linux
  - [ ] Virtual file system
  - [ ] musl support
  - [ ] fork/exec
  - [ ] signal
  - [ ] tty
  - [ ] pipe
  - [ ] shell
  - [ ] e2e testing with TypeScript
- [ ] **Milestone: Run FTL's own website on FTL [like this](https://seiya.me/blog/new-microkernel-os-in-10-days)**
  - [ ] Device driver framework
  - [ ] Virtio-netfutexz
  - [ ] TCP/IP networking
  - [ ] Google Compute Engine support
- [ ] **Milestone: Support modern software**
  - [ ] Node.js (epoll, futex, ...)
- [ ] **Milestone: Make it operational**
  - [ ] Good sysadmin tools for FTL
  - [ ] Interceptors

## Design

FTL is a hybrid kernel. It has a small core (like microkernel) with language-isolated OS components (like [TockOS](https://dl.acm.org/doi/10.1145/3131672.3136988), [RedLeaf](https://www.usenix.org/conference/osdi20/presentation/narayanan-vikram), and [framekernel](https://www.usenix.org/conference/atc25/presentation/peng-yuke)).

### Secure kernel without compromise

Similar to microkernels, most of OS services such as device drivers, file systems, and network stacks, and Linux compatibility layer are implemented as isolated OS services (servers) on top of the kernel.

Servers coexist in the kernel space with language-based isolation, relying on Rust's safety guarantees and sound and securely designed API for OS services. Language-based isolation is weaker than hardware-based ones used in traditional microkernels, but it enables _good-enough_ security without sacrificing performance.

### Per-container kernels

FTL implements OS personality as a server. This lets you run Linux containers on their own isolated Linux-like application kernels, similar to [gVisor](https://github.com/google/gvisor), but without the overhead of system call hooking.

You can also update the Linux-like application kernel simply by starting a new container, without rebooting the machine.

The application kernel is implemented as a Rust library crate, with a clear interface for interacting with the kernel core. We hope this will enable complicated features such as process snapshotting and container live migration in the future.

### Linux compatibility layer

FTL is designed to support multiple personalities. The primary personality is the Linux compatibility layer, which allows running Linux binaries without any modifications.

Foreign binary support (ABI emulation) is a well-established technique that can be seen in modern operating systems. Windows Subsystem for Linux (WSL 1), FreeBSD's [Linuxulator](https://wiki.freebsd.org/Linuxulator), [OSv](https://osv.io/), to name a few.

The key difference in FTL is that each container has its own isolated Linux-like application kernel, implemented on top of a small kernel core.

We also aim to add our own personality to offer experimental system calls and features not available in Linux.

### Interceptors (planned)

Interceptor is a planned feature to control the behavior of OS components at runtime, just like middlewares in web frameworks. Rate limiting, security auditing, network packet routing, live patching, will be implemented as interceptors.

### Batteries included

FTL will be more similar to BSD than Linux. We plan to provide FTL as a minimalistic OS with userspace utilities integrated nicely. This will include at least: kernel, OS servers, init system, container management, cloud platform integration, and some basic utilities like shell.

## License

FTL is dual-licensed under the MIT and Apache 2.0 licenses.
