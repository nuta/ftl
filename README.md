# FTL

FTL is a new general-purpose operating system based on a modern microkernel architecture. It is designed to provide the best developer experience so that you, even a kernel newbie, can easily understand and enjoy developing an operating system.

- **[Quickstart](docs/quickstart.md)**: Setting up the development environment, and running FTL on QEMU emulator.
- **[Writing Your First Application](docs/guides/writing-your-first-application.md)**: Step-by-step guide for writing your first FTL application.
- **[Guides](docs/guides)**: More step-by-step guides for developing OS components such as device drivers.

## Why FTL?

*"What if we try building a microkernel-based general-purpose operating system with 21st century technologies?"* This is the question we try to answer. There are already many microkernel projects out there, however, they aim to be hobby/research projects or designed for embedded systems.

It has been said that microkernels are not practical due to performance overhead, but the hardware and software landscape has changed a lot since the 1990s. Don't you think it's time to revisit the microkernel architecture? Let's try with modern technologies and see how far we can go!

FTL aims to be:

- **Approachable:** Be easy to understand and develop, even for non-experts. Have you ever experienced some programming languages? You are ready to join the development!
- **Practical:** Aim to be a general-purpose operating system, not ending up as a hobby/research project.
- **Performant:** Don't stick to the beautiful design.

## Design Principles

To achieve this goal, we have the following design principles:

- Aim to be easy to develop, not achieving a correct and beautiful architecture. Make OS development approachable and fun for everyone.
- Don't try to achieve the perfect design from the beginning. Imagine how the userspace should look like first, not vice versa - the microkernel is just a runtime for applications.
- The traditional "user-mode" concept is just one of the many ways to isolate components. Implement other faster ways like language-based (e.g. Rust/WebAssembly), or Intel/Arm specific mechanisms (e.g. Intel PKS) for better performance.
- Implement in [Rust](https://www.rust-lang.org/) with async APIs, without async Rust (`async fn`). Every component has a simple main loop to make the execution flow clear.

TODO: nice drawing of the architecture

## Features

- 64-bit RISC-V support.
- Virtio device support (virtio-net only for now).
- TCP/IP stack based on [smoltcp](https://github.com/smoltcp-rs/smoltcp).
- Intuitive Rust API for apps, OS servers, and device drivers.
- Auto-generated IPC stubs and startup code from declarative YAML files.

### Planned Features


- MMU/usermode-based isolation (so-called traditional "process" isolation).
- Arm and x86_64 support (once the kernel API stabilizes).
- Shell.
- File system support.
- JavaScript API.
- WebAssembly-based isolation.
- Linux compatibility layer using the genuine Linux kernel microVM.

## Getting Started

See [Quickstart](docs/quickstart.md) for the quick start guide.

## License

FTL is dual-licensed under [MIT license](https://opensource.org/license/mit) and [Apache 2.0 license](https://opensource.org/license/apache-2-0).
