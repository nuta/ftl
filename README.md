# FTL

FTL is a new general-purpose operating system based on a modern microkernel architecture. It is designed to provide the best developer experience so that you, even a kernel newbie, can easily understand and enjoy developing an operating system.

## Why FTL?

"What if we try building a microkernel-based general-purpose operating system with 21st century technologies?" This is the question we try to answer. There are already many microkernel projects out there, however, they aim to be hobby/research projects or designed for embedded systems.

It has been said that microkernels are not practical due to performance overhead, but the hardware and software landscape has changed a lot since the 1990s. Don't you think it's time to revisit the microkernel architecture? Let's try with modern technologies and see how far we can go!

- Make OS development approachable and fun for everyone. Aim to being easy to develop, not achiving a correct and beautiful architecture.
- Make it work with ergonomic APIs first, iterate on it, and then make it performant.
- Implement in [Rust](https://www.rust-lang.org/) with async APIs, in a simple event-driven design without `async fn` - write a event loops manually.

# Getting Started

> [!NOTE]
> Prerequisites: You need to install Cargo ([rustup](https://rustup.rs/)) and QEMU.
>
> ```
> brew install qemu             # macOS
> sudo apt install qemu-system  # Ubuntu
>
> rustup target add TODO:
> rustup component add rust-src TODO:
> ```

```
git clone https://github.com/nuta/ftl
cd ftl
make run
```
