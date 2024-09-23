# FTL

FTL is a new general-purpose operating system based on a modern microkernel architecture. It is designed to provide the best developer experience so that you, even a kernel newbie, can easily understand and enjoy developing an operating system.

TODO: nice drawing of the architecture

- **[Quickstart](docs/quickstart.md)**: Setting up the development environment, and running FTL on QEMU emulator.
- **[Writing Your First Application](docs/guides/writing-your-first-application.md)**: Step-by-step guide for writing your first FTL application.
- **[Guides](docs/guides)**: More step-by-step guides for developing OS components such as device drivers.

## Why FTL?

*"What if we try building a microkernel-based general-purpose operating system with 21st century technologies?"* This is the question we try to answer. There are already many microkernel projects out there, however, they aim to be hobby/research projects or designed for embedded systems.

It has been said that microkernels are not practical due to performance overhead, but the hardware and software landscape has changed a lot since the 1990s. Don't you think it's time to revisit the microkernel architecture? Let's try with modern technologies and see how far we can go!

FTL aims to be:

- **Approachable:** Be easy to understand and develop, even for non-experts. Focused on the developer experience.
- **Practical:** Aim to be a general-purpose operating system, not ending up as a hobby/research project.a
- **Performant:** The traditional "user mode" concept is just one of the many ways to isolate components. Implement other faster ways like language-based (e.g. Rust/WebAssembly), and CPU-specific mechanisms (e.g. Intel PKS) for better performance.

## Design Principles

To achieve this goal, we have the following design principles:

- **Userspace first:** FTL started from designing the developer experience in userspace - *"How do I want to write OS components?"*. In FTL, the kernel is just a runtime for applications.
- **Intuitiveness over perfection:** Don't stick to a *correct* design first. Deliver an intuitive, easy-to-use, API good enough for the 90% of the use cases. The remaining 10% can be solved later, with specialized APIs. Make it work first, then make it better.
- **Convention over configuration:** Microkernel-based OS is a collection of components. They should look similar and be easy to understand. Follow conventions to focus on what you want to do, and reduce the cognitive load of new contributors.

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
