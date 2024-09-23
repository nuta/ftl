# FTL

FTL is a new general-purpose operating system based on a modern microkernel architecture. It is designed to provide the best developer experience so that you, even a kernel newbie, can easily understand and enjoy developing an operating system.

TODO: nice drawing of the architecture

- **[Quickstart](docs/quickstart.md)**: Setting up the development environment, and running FTL on QEMU emulator.
- **[Writing Your First Application](docs/guides/writing-your-first-application.md)**: Step-by-step guide for writing your first FTL application.
- **[Guides](docs/guides)**: More step-by-step guides for developing OS components such as device drivers.

## Why FTL?

***"What if we try building a microkernel-based general-purpose OS with 21st century technologies?"*** This is the question we try to answer. There are already many microkernel projects out there, however, they aim to be hobby/research projects or designed for embedded systems.

It has been said that microkernels are not practical due to performance overhead, but the hardware and software landscape has changed a lot since the 1990s. Don't you think it's time to revisit the microkernel architecture? Let's try with modern technologies and see how far we can go!

FTL aims to be:

- **Approachable:** Be easy to understand and develop, even for non-experts. Focused on the developer experience.
- **Practical:** Aim to be a general-purpose operating system, not ending up as a hobby/research project.
- **Performant:** Moving beyond traditional "user mode" for component isolation. Implementing faster methods like language-based (e.g., Rust/WebAssembly) and CPU-specific mechanisms (e.g., Intel PKS) for enhanced performance.

## Design Principles

To achieve this goal, we have the following design principles:
To achieve these goals, we adhere to the following design principles:

- **Userspace over kernel-space:** FTL's design process began with the developer experience in userspace - asking, "How do we want to write OS components?" In FTL, applications take center stage, with the microkernel provindng the necessary abstractions to support them.
- **Intuitiveness over perfection:** Prioritize delivering an intuitive, easy-to-use API that addresses 90% of use cases, rather than striving for an initially "perfect" design. The remaining 10% can be solved later with specialized APIs. Our approach is to make it work first, then make it better.
- **Convention over configuration:** As a microkernel-based OS, FTL is a collection of components. These should be consistent and easily understandable. By following conventions, we reduce the cognitive load on new contributors and allow developers to focus on their specific tasks.

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
