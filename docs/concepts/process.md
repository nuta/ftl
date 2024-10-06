---
title: Process, Thread, and Isolation
---

Like popular operating systems, each running program instance (or application) in FTL is represented as a *process*.

Process can be considered as a container of other kernel objects such as:

- Virtual memory space (`vmspace` object).
- System resources (`handle` object).
- Threads of execution (`thread` object).

> [!TIP]
>
> Unlike Linux, processes don't have parent-child relationships in FTL. Instead, they are in a flat hierarchy.

## Virtual Memory Space

In Linux, each process has its own address space. In FTL, on the other hand, it's slightly different - each process belongs to a *virtual memory space*. This means that multiple processes can share the same address space! It enables to implement a lightweight IPC mechanism without memory copies nor kernele involvement (not implemented yet though).

## Handles

Each process has a set of *handles* to access kernel objects. You might be familiar with the concept of file descriptors in Linux. Like you specify a file descriptor to access a file in Linux, you specify a handle to access a kernel object in FTL.

## Threads

Threads are execution units sharing the same resources, that is, a process. Thread is not something special at all in FTL, but it's more lightweight than other operating systems.

## Isolation

In major operating systems, processes are isolated based on CPU's user mode mechanism. In FTL, we support broader isolation mechanisms for performance:

- **language-based:** Trusting the language's memory safety guarantees. E.g., Rust.
- **user mode:** CPU's user mode mechanism (to be implemented).

Isolation mode can be decided by users. For example, you can run a process in user-mode isolation as you do in Linux, or you can run it in the kernel process (see below) for performance, if you can trust the program. It's seamless and easy to switch between isolation levels (user mode is not yet implemented as of now, but it will be!).

In the future, we plan to support more isolation mechanisms such as WebAssembly-based isolation, and hardware-based isolation (e.g. Intel VT-x). Stay tuned!

> [!TIP]
>
> **Design decision: Compromise between performance and security.**
>
> In FTL, we aim to achieve high-performance and high-reliability. This *isolation mode* concept allows users to choose the right balance between performance and security. Ideally, if you write an app in Rust without `unsafe`, it should be safe to run in the kernel mode. Also, if API is designed properly, you can switch between isolation modes without changing the code.

## Kernel Process

Kernel process is a special process that runs in the kernel space. It's designed for performance-critical tasks.

In the kernel process, by nature, usermode isolation cannot be used. Instead, processes are isolated with language-based or, in the future, WebAssembly-based mechanisms.
