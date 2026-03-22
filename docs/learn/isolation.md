# Isolation

Isolation is a mechanism to isolate a set of processes from others. In other words, an isolation defines how memory safety is enforced for the belonging processes.

> [!NOTE]
> **Design decision: High performance with reasonable security**
>
> With the unikernel-style isolation described below, your apps will run like user-level threads. FTL API hides the underlying implementation details, and you can write apps that work in both user and kernel modes.
>
> This approach relies on that:
>
> - Rust compiler guarantees the memory safety.
> - All dangerous APIs that can break the memory safety are marked as `unsafe`.
> - The application is compiled with `#![forbid(unsafe_code)]`, or you review the `unsafe` code.
>
> This means you need to trust the applications, however, it is reasonably realistic by using Rust to enforce memory safety guarantees at compile time.

## Mechanisms

In traditional operating systems, the concept of isolation is hard-coded using the page table (memory management unit).

### Unikernel-style Isolation

In short, trust Rust's memory safety guarantees. In this style, processes run in the kernel memory space, and system calls are just function calls to the kernel's handler.

This isolation is suitable for components that are performance critical, and you can trust providers of these components (i.e. well-written and not malicious).

> [!WARNING]
> **Writing in Rust does not mean it's secure!**
>
> Logic errors, memory leaks, deadlocks, and runtime errors (`panic!`) are out of scope of Rust's safety guarantees. It means you need to trust the reliability of the applications you run in the unikernel-style isolation.
>
> Another common misconception is that memory safety is unique to Rust. In fact, JavaScript and other languages with a garbage collector are generally immune to memory safety issues.
>
> Why Rust then? Because it provides memory safety with zero cost abstraction (to be more precise, *"pay-as-you-go"* cost). This is why it shines in performance-critical software like operating systems.

### User Mode Isolation

> [!CAUTION]
> **Not Yet Implemented:** The feasibility of this feature is confirmed in PoC, but it's not yet implemented. Stay tuned.

This is the traditional microkernel style isolation. Each isolation has its own page table, and programs can't perform privileged instructions.

This isolation is suitable for components that are not Rust-based, not battle-tested yet, or not performance critical.

### WebAssembly or GC-based Isolation (JavaScript in Kernel)

> [!CAUTION]
> **Not Yet Implemented:** This is not available yet. Consider this section as a a vision of the future.

Run a trusted runtime in the kernel space. The most promising candidate is [MicroQuickJS](https://github.com/bellard/mquickjs), a small JavaScript runtime designed for constrained devices.

This isolation relies on the runtime's memory safety guarantees. That is, the runtime (MicroQuickJS) guarantees applications can't access memory out of bounds or perform other unsafe operations.
