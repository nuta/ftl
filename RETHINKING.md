# Rethinking Microkernels for general-purpose OS: Finer-grained components for performance & DX

## "microkernels are slow"

Microkernel is a design of operating system kernel. Unlike traditional monolithic kernels, microkernels provide only few primitives: threads, memory management, inter-process communication (IPC), interrupt handling, for example. In an extreme case, [seL4](https://docs.sel4.systems/Tutorials/untyped.html) doesn't allocate memory but userland tell the kernel where to use. This [separation of mechanism and policy](https://en.wikipedia.org/wiki/Separation_of_mechanism_and_policy) is a key design principle of microkernels and make them frameworks of operating systems.

Microkernels have succeeded very well, without a doubt. A famous real world example is [Apple's L4-based microkernel](https://support.apple.com/fr-cm/guide/security/sec59b0b31ff/web#:~:text=Apple%2Dcustomized%20version%20of%20the%20L4%20microkernel) running on iPhone's Secure Enclave. Microkernels suit very well for components that require high security and reliability.

However, have you ever tried running microkernels on your desktop? What about on your web app server? I bet you haven't. Why? Because microkernels are (considered) slow. In 20th century, before I was born, some microkernel-based OSes were released but people ended up using using hybrid/monolithc kernels such as Windows, XNU (macOS), and Linux.

They're indeed great, but I came up with a question: "what if we redesign a microkernel-based general-purpose OS in 2024? Can we make it competitive with, or even faster than, Linux?"

Microkernel authors are digging deeper and deeper into reliability and ultimate policy-free kernel design.

People say "microkernels are slow" but also say "microkernels are beautiful and intruiging design". What if we write a general-purpose OS again in 2024, with the modern technology? Is it worth replacing existing OSes?

What's more, some people think kernel is slow, or want to inject code.

- Snap (Google)
- ghost (Google)
- eBPF

## Five design principles towards general-purpose microkernels

- Performance: many-core, nano-services
- Flexibility: Nano-services
- Security: Nano-services
- Compatibility: unmodified Linux
- Contributors: API-first, Neat DX

### Design OS as a distributed system

- Barrelfish
- Adopt many core era: async, shared-nothing, and zero-copy


### Nano-services: process is just one of isolation mechanism

### API-first design: microkernel with compromises


### Neat DX: develop OS as if it's a web app

API is just one of the interfaces humans use. We should also consider the whole development experience (DX) of OS development. It should be easy, productive, and most importantly, fun!

- Good devtools
- Good o11y
- Good documentation
- K8s for OS
- Rust

### Don't emulate everything: use real Linux to run Linux binaries

In 2021, I wrote [Kerla](https://seiya.me/blog/writing-linux-clone-in-rust), a new monolithic kernel with Linux binary compatibility written in Rust. It was fun but I find it painful

mcKernel

Not sure if this is a good idea, but it's worth trying.
