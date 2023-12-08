# FTL

## Motivation

Ruby on Rails, but for developing microkernel operating systems.

## Design

- **API-first design:** I don't think FTL has invented anything new. I wouldn't write an academic paper on it. It's just a collection of existing ideas that have been put together in a neat way. FTL started with a simple question: "how would I want to write userspace programs if I were designing an microkernel OS from scratch?"
- **Nanoservices:** For example, TCP/IP server would be a single process in a traditional microkernel OSes, but in FTL, it's a collection of finer-grained services: TCP, UDP, IPv4, ARP components are all separate independent processes (Fibers) that communicate with each other via explicit ways. This makes implementation, testing, and debugging much easier.
- **Transparent hardware/language-based isolation:** Nanoservices sound attactive, but they are not practical due to overhead of inter-process communication. However, what if we can choose to run them in the same address space, as lightweight threads like Goroutines?
- **Kubernetes-like declarative management & neat observability:** Who wants to manage completely new operating systems in, for example data centers? It should be very easy and fun to deploy, manage, and monitor.

## Implementation principles

- **Manual asynchronous programming without `async`/`await`:** This might change in the future, but for now, FTL doesn't support `async` Rust. Instead, asynchronism in nanoservices are implemented as explicit state machines. Interestingly, because each nanoservice is very small, the implementation is still super simple and is easier to understand.

## Primitives

- `Fiber`:
- `Channel`:
- `EventQueue`:
- `Signal`:
- `OwnedPage` | `SharedPage`:

## TODOs

- Tests in std
- io_uring
- Erlang
- Channel message transaction ID

- IDL
- Environ & Houston
- Benchmarks
- Kernel
- Run on GCE w/ great o11y
- WASM-based playground

- How async Rust works
