# Comparisons with Others

*"How's FTL different from X?"* is the question you'll probably ask first. In this article, we will explore the unique features, design choices, advantages, and most importantly, the disadvantages of FTL compared to other microkernels.

This kind of article is uncomfortable to write because some use this kind of comparison for a marketing battle, or FUD. Thus, I'd make it clear that ***"it depends"***. Differences are why many microkernels, text editors, programming languages, and ramen restaurants exist in this world.

If you found something wrong or inaccurate, please open an issue or a pull request. I will be happy to correct it :)

## Userspace-First Design

This is vague but the most important philosophy of FTL. FTL is designed to be a userspace-first microkernel, which means we prioritize how we want to develop applications and OS components in userspace, instead of achieving the most ideal kernel design. That is, **developer experience is the top priority**.

This means the kernel may sometimes have some nasty hacks to make things work for now. For example, the current kernel does dynamic memory allocation in the kernel, which is less ideal compared to other strict microkernels. However, we prefer to keep it intuitive for newbies until we really have to optimize it. That is, we make it work first, and make it better incrementally.

The opposite of this is what I call "kernel-first" design. seL4 is a good example of this. seL4 is an extremely strict design. I'm saying *strict* not because it's formally verified, but because [its API](https://docs.sel4.systems/projects/sel4/api-doc.html) is super minimal. You may notice that it exposes low-level hardware details directly (e.g. `seL4_X86_PageTable` and `seL4_ARM_PageTable`) and has no dynamic memory allocation API. This lack of abstractions makes the kernel minimal, and gives you the freedom to implement your own abstractions. FTL is the opposite of this. It tries to hide kernel's implementation details even if it sacrifices some performance.

## Multiple Isolation Mechanisms

Microkernel is a design pattern where the kernel is as small as possible, and everything else is implemented as user-space processes. In so-called multi-server microkernels, the userland OS components are implemented as separate processes. For example, TCP/IP process, file system, and each device driver have their own process.

Separate processes here means that they are isolated from each other, as in they cannot access each other's memory nor other kernel resources (e.g. file descriptors). This makes the system more secure and stable, as a bug in one process cannot crash the whole system. This is called "process isolation", and is a key feature of microkernels.

Traditionally, process isolation is achieved by virtual memory, aka. paging. Each process has its own virtual address space, and the CPU enforces this isolation. OS components communicate with each other using IPC (Inter-Process Communication) mechanisms, such as message passing and shared memory. Since monolithic kernels do function calls instead of IPC, it's intuitive to think that microkernels are slower than monolithic kernels due to IPC overheads.

In FTL, process isolation can be done in different ways, depending on your needs. Along with usermode isolation, FTL plans to support:

- **In-kernel Rust-based isolation:** Trust [safe Rust](https://doc.rust-lang.org/nomicon/safe-unsafe-meaning.html) code to be memory safe and use Rust's type system to enforce isolation. This enables super lightweight processes as they're embedded in the kernel. Good enough isolation for trusted components.
- **In-kernel JavaScript-based isolation:** Run a trusted language runtime in the kernel space. This can be any other GC-based languages.
- **In-kernel WebAssembly-based isolation:** Use in-kernel WebAssembly engine to guarantee memory safety and isolation. This is a good option for untrusted components, and is also nice for porting existing [WASI-based](https://wasi.dev/) applications.

Why multiple isolation mechanisms? Because it always depends on the use case. For example, you can trust core components like the official TCP/IP server and run it in kernel space for performance, while running device drivers written in C in usermode for reliability, and eventually run untrusted potentially-malicious code in VMM-based isolation for security in the future.

## Schema-less Message Passing

Message passing is a major IPC mechanism in microkernels. It's similar to UNIX domain socket but in a datagram-like way.

Typical microkernels (and so does FTL) do not parse the message contents, but treat it as an opaque byte array. This means that the sender and receiver must agree on the message format, which is usually done using an Interface Definition Language (IDL). For example, Fuchsia uses its own IDL called [FIDL](https://fuchsia.dev/fuchsia-src/concepts/fidl/overview).

FTL uses message passing for IPC, without IDL. Instead, it has a predefined set of message types. This sounds like moving backwards, but it actually has some advantages:

- **No new language to learn:** You don't need to learn a new IDL language. Just learn the few predefined message types.
- **No code generation:** IPC stubs are not necessary. This makes the code simpler and easier to read and debug.
- **Speed:** We can optimize the message passing for the predefined message types.
- **Composability:** You'll be able to compose apps like piping UNIX commands (`cat | grep | wc`) thanks to the uniform interface.

To summarize, FTL has *"everything is a file"*-like philosophy in message passing. That is, we prefer a simple interface which covers 90% of the use cases, instead of having specialized interfaces for each use case. A key finding here is that interactions between OS components are way simpler than gRPC-powered applications.

> [!NOTE]
> Learn more in [Channel](./learn/channel).
