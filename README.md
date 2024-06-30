# FTL (codename)

FTL is a completely new microkernel-based operating system designed for best developer experience.

- **Microkernel + nano-services architecutre.** FTL is a microkernel-based OS. Most OS functionalities (file system, TCP/IP, device drivers) are implemented as *nanoservices*, lightweight processes that communicate with each other via message passing. In FTL, services are finer-grained than traditional microkernel-based OSes: for example, TCP/IP stack would be implemented as a combination of IP, TCP, ARP nanoservices. Imagine goroutines in Go apps, but isolated from each other.
- **API-first design.** FTL prefers intuitive API over minimality and [policy-freedom](https://en.wikipedia.org/wiki/Separation_of_mechanism_and_policy) of the kernel. Designed for developer happiness.
- **Process is just one of isolation mechanisms.** Traditonally, userspace is the natural boundary of processes in the microkernel. In 2024, we have more isolation mechanisms: e.g., language-based (Rust), runtime-based (WebAssembly), virtualization-based (Intel VT-x, AMD-V, ARM TrustZone), and more! If we can trust your app and Rust's safety, why not run apps in the same address space, even in the kernel space?
- **Kubernetes-like declarative userpace:** Write spec
- **Written entirely in Rust.** Buuuut we plan to support other languages for userspace development!

# Motivation

Let's invent Ruby on Rails in the operating system world. I love kernel hacking, but it feels like web development in the 00s. Today's OS development is still too low-level and is scary for beginners. Also, despite one of microkernel's promises is to make the OS more modular, maintainable, and easier to develop, only limited enthusiastic people enjoy the benefits

What if we can make OS development as easy as web development? That's why I started FTL.

# Will FTL replace Linux?

Never! Use Linux. Full stop.

Linux is battle-tested, well-docoumented, extensive ([eBPF]), and can be microkernel-ish ([Userspace I/O], [Snap], [ghOSt]) if you want. Of course FTL will be production ready and achieve v1.0, but

[eBPF]: https://ebpf.io/
[Userspace I/O]: https://www.kernel.org/doc/html/v5.0/driver-api/uio-howto.html
[Snap]: https://research.google/pubs/snap-a-microkernel-approach-to-host-networking/
[ghOSt]: https://research.google/pubs/ghost-fast-and-flexible-user-space-delegation-of-linux-scheduling/

# Areas

- Datacenter network apps
- Embedded

