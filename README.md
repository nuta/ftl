# FTL

FTL is a hybrid kernel-based operating system aiming to be the drop-in third choice, after Linux and BSDs.

- **Secure:** Simple and small kernel which implements hypervisor-shaped minimalistic system calls, per-container userspace OS, and more proactive security measures.
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

- **Milestone: Make shell work (work-in-progress)**
  - [x] Kernel: vCPU and memory management
  - [ ] System call emulation: Hello World from Linux binary
  - [ ] musl libc support
  - [ ] Virtual file system
  - [ ] fork/exec
  - [ ] signal
  - [ ] tty
  - [ ] pipe
  - [ ] shell
  - [ ] e2e testing with TypeScript
- **Milestone: Run FTL's own website on FTL [like this](https://seiya.me/blog/new-microkernel-os-in-10-days)**
  - [ ] Device driver framework
  - [ ] Virtio-net
  - [ ] TCP/IP networking
  - [ ] Google Compute Engine support
- **Milestone: Support modern software**
  - [ ] Node.js (epoll, futex, ...)
- **Milestone: Make it operational**
  - [ ] Good sysadmin tools for FTL
  - [ ] Interceptors

## Design

### Per-container userspace OS

FTL kernel provides only hypervisor-like interfaces such as vCPU and VM space (virtual address space). Most of OS features such as the concept of process is implemented in a userspace library, which we call *userspace OS*.

Each container instance has its own isolated userspace OS. This lets you run Linux containers on their own isolated Linux-like application kernels, similar to [gVisor](https://github.com/google/gvisor).

You can upgrade, add your own features, inject `printf`s for debugging, or optimize the userspace OS by simply starting a new container, without rebooting the machine. We hope this will enable complicated features such as process snapshotting and container live migration in the future.

### Linux compatibility layer

FTL is designed to support multiple personalities. The primary personality is the Linux compatibility layer, which allows running Linux binaries without any modifications.

Foreign binary support (ABI emulation) is a well-established technique that can be seen in modern operating systems, such as Windows Subsystem for Linux (WSL 1), FreeBSD's [Linuxulator](https://wiki.freebsd.org/Linuxulator), and [OSv](https://osv.io/), to name a few.

The key difference in FTL is that each container has its own isolated Linux-like application kernel, implemented on top of a small kernel core.

We also aim to add our own personality to offer experimental system calls and features not available in Linux.

### Interceptors (planned)

Interceptor is a planned feature to control the behavior of OS components at runtime, just like middlewares in web frameworks. Rate limiting, security auditing, network packet routing, and live patching will be implemented as interceptors.

### Batteries included

FTL will be more similar to BSD than Linux. We plan to provide FTL as a minimalistic OS with userspace utilities integrated nicely. This will include at least: kernel, OS servers, init system, container management, cloud platform integration, and some basic utilities like shell.

## License

FTL is dual-licensed under the MIT and Apache 2.0 licenses.
