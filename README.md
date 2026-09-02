# FTL

FTL is a new operating system aiming to be the drop-in third choice, next to Linux and BSDs.

```
Container #1           Container #2
┌─────────────────┐   ┌────────────────────────────────┐
│ VmSpace         │   │  VM space       VM space       │
│ ┏━━━━━━━━━━━━━┓ │   │ ┏━━━━━━━━━━━━┓  ┏━━━━━━━━━━━━┓ │
│ ┃             ┃ │   │ ┃            ┃  ┃            ┃ │
│ ┃ FTL native  ┃ │   │ ┃   Linux    ┃  ┃   Linux    ┃ │
│ ┃  unikernel  ┃ │   │ ┃  Process   ┃  ┃  Process   ┃ │
│ ┃             ┃ │   │ ┃            ┃  ┃            ┃ │
│ ┃╌╌╌╌╌╌╌╌╌╌╌╌╌┃ │   │ ┃ ╌╌╌╌╌╌╌╌ Linux ABI ╌╌╌╌╌╌╌╌┃ │
│ ┃  Your own   ┃ │   │ ┃   Linux compat library     ┃ │
│ ┃  custom OS  ┃ │   │ ┃  (Process, VFS, TCP, ...)  ┃ │
│ ▝▀▀▀▀▀▀▀▀▀▀▀▀▀▘ │   │ ▝▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▘ │
└─────────────────┘   └────────────────────────────────┘
                                               Userspace
╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶
                                                  Kernel
╔══════════════════════════════════════════════════════╗
║                    Hybrid kernel                     ║
║        (memory, vCPU, network multiplexing, ...)     ║
╚══════════════════════════════════════════════════════╝
```

- **Secure:** A small kernel provides minimalistic hypervisor-shaped system calls. OS features such as Linux system calls and TCP are implemented in a userspace library called _userspace OS_.
- **Linux compatible:** FTL aims to be a drop-in alternative to Linux in the cloud environment. Linux binary compatibilty layer is implemented as a library.
- **Programmable:** Key OS features are in a userspace OS library. You can write your OS just like [unikernels](https://en.wikipedia.org/wiki/Unikernel), simply by patching the library (without kernel programming).
- **Approachable:** Easy-to-understand and testable codebase, handy end-to-end testing in TypeScript, and quick edit-compile-run cycle.
- **Lightweight:** Keep its footprint small to run even on constrained devices, and to develop the OS quickly.

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

You can also run FTL on cloud such as Google Compute Engine. See the [deploy script](tools/deploy-to-google-cloud.sh).

## Design

### Per-container userspace OS

Most of OS features such as the concept of Linux process is implemented in a userspace library, which we call _"userspace OS"_. This design is similar to [Exokernel](https://dl.acm.org/doi/10.1145/224057.224076) where the traditional monolithic kernel features are moved to a userspace library (library OS).

FTL kernel provides hypervisor-like primitives: vCPU (thread), virtual address space (vmspace), virtual network, for example. The kernel does not interpret Linux system calls, and redirects them to the userspace OS's handler.

Each container instance has its own isolated userspace OS. This lets you run Linux containers on their own isolated Linux-like application kernels, similar to [gVisor](https://github.com/google/gvisor).

You can upgrade, add your own features, inject `printf`s for debugging, or optimize the userspace OS by simply starting a new container, without kernel programming or rebooting the machine.

### Linux compatibility layer

FTL is designed to support multiple personalities. The tier 1 personality is Linux which runs Linux executables without any modifications.

Foreign binary support (aka. ABI emulation) is a well-established technique that can be seen in modern operating systems, such as Windows Subsystem for Linux (WSL 1), FreeBSD's [Linuxulator](https://wiki.freebsd.org/Linuxulator), and [OSv](https://osv.io/), to name a few.

The key difference in FTL is that it is implemented as a userspace library, not as a in-kernel feature separate supervisor process. Each FTL container has its own isolated Linux-like application kernel.

We also aim to add our own personality to offer FTL's own system calls and features not available in Linux.

### Interceptors (planned)

Interceptor is a planned feature to control the behavior of OS components at runtime, just like middlewares in web frameworks. Rate limiting, security auditing, network packet routing, and live patching will be implemented as interceptors.

## License

FTL is dual-licensed under the MIT and Apache 2.0 licenses.
