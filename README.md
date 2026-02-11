# FTL

FTL is a microkernel-based operating system built for developers, aiming to be production-ready. It prioritizes developer experience and ease of understanding, making OS development feel like writing a web application.

## Core Principles

FTL follows three key design philosophies:

- **Userspace-first:** Make OS development approachable and fun for everyone. Prioritize developer experience in the userspace, where the most OS components reside. The microkernel is just a runtime for applications.
- **Simplicity over perfection:** Focus on straightforward solutions that handle 99% cases. Make it work first. Make it better later.
- **Incrementally adoptable:** Designed to work seamlessly with existing systems for gradual adoption.

## Running OS in QEMU

Install [Rust toolchain](https://rustup.rs/), [Bun](https://bun.com/docs/installation), [QEMU](https://www.qemu.org/download/#macos), and run:

```
bin/ftl run
```

See [Getting Started](docs/getting-started.md) for more details.

## License

FTL is dual-licensed under the MIT license and the Apache License (Version 2.0).
