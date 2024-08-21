# FTL

FTL is a new general-purpose operating system based on a modern microkernel architecture. It is designed to provide the best developer experience so that you, even a kernel newbie, can easily understand and enjoy developing an operating system.

## Why FTL?

"What if we try building a microkernel-based general-purpose operating system with 21st century technologies?" This is the question we try to answer. There are already many microkernel projects out there, however, they aim to be hobby/research projects or designed for embedded systems.

It has been said that microkernels are not practical due to performance overhead, but the hardware and software landscape has changed a lot since the 1990s. Don't you think it's time to revisit the microkernel architecture? Let's try with modern technologies and see how far we can go!

FTL aims to be:

- **Simple:** Easy to understand and develop, even for non-experts.
- **Pratical:** Aim to be a general-purpose operating system, not ending up as a hobby/research project.
- **Performant:** Don't stick to the beautiful well-isolated architecture.

## Design Principles

To achieve this goal, we have the following design principles:

- Aim to being easy to develop, not achiving a correct and beautiful architecture. Make OS development approachable and fun for everyone.
- Don't try to achieve the perfect design from the beginning. Imagine how the userspace should look like first, not vice versa - the microkernel is just a runtime for applications.
- "process" is just one of the many ways to isolate components. Implement recent ways like language-based (Rust/WebAssembly), or Intel/Arm specific mechanisms (Intel PKS) for better performance.
- Implement in [Rust](https://www.rust-lang.org/) with async APIs, without async Rust (`async fn`). Every component has a simple main loop to make the execution flow clear.

TODO: nice drawining of the architecture

## Features

- 64-bit RISC-V support
- 64-bit Arm support (partially)
- Virtio device support (virtio-net only for now)
- TCP/IP stack based on [smoltcp](https://github.com/smoltcp-rs/smoltcp)
- Intuitive Rust API
- Auto-generate IPC stubs and startup code from declarative YAML files

- MMU/Usermode-based isolation (planned - so-called "process" isolation)
- Shell (planned)
- File system support (planned - *"SQLite3 as a file system"*)
- JavaScript API (planned)
- WebAssembly support (planned)
- Linux compatibility layer using the genuine Linux kernel (planned)

## Getting Started

**Prerequisites:** [Rust toolchain](https://rustup.rs/) and [QEMU](https://www.qemu.org/download/#macos) on macOS or Linux.

1. Clone the repository and move to the directory.

```
git clone
cd ftl
```

2. Try running it.

```
make run
```

That's it! Follow the following steps to learn more about FTL.

## Directory Structure

| Path | Description |
| --- | --- |


## License

FTL is dual-licensed under [MIT license](https://opensource.org/license/mit) and [Apache 2.0 license](https://opensource.org/license/apache-2-0).
