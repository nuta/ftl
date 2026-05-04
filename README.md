# FTL

FTL is a microkernel-based operating system built for developers, aiming to be production-ready. It prioritizes developer experience and ease of understanding, making OS development feel like writing a web application. It designed to be:

- **Userspace first:** Make OS development approachable and fun for everyone. Prioritize developer experience in the userspace, where the most OS components reside. The microkernel is just a runtime for applications.
- **Simple over perfect:** Focus on straightforward solutions that handle 99% cases. Make it work first. Make it better later.
- **Incrementally adoptable:** Designed to work seamlessly with existing systems for gradual adoption.

## Features & Roadmap

- [x] Microkernel: asynchronous message passing + event stream API.
- [x] In-kernel isolation for performance-critical components
- [x] Sandboxed processes for ABI emulation
- [x] Virtio-net device driver
- [x] smoltcp-based TCP/IP stack server
- [ ] Our own TCP/IP stack implementation (WIP)
- [ ] Async Rust support (WIP)
- [ ] Cachefs: filesystem for ephemeral read-heavy data
- [ ] Linux compatibility layer

## Quickstart

Install [Rust toolchain](https://rustup.rs/), [Bun](https://bun.com/docs/installation), [QEMU](https://www.qemu.org/download/#macos), and run:

```
bin/ftl run
```

See [Getting Started](docs/getting-started.md) for more details.

## License

FTL is dual-licensed under the MIT license and the Apache License (Version 2.0).
