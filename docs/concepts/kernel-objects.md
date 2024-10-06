---
title: Kernel Objects
---

FTL kernel is a collection of kernel objects, such as processes, threads, channels, and memory spaces. Let's dive into the concepts of FTL to understand its design and philosophy.

> [!TIP]
>
> **Design decision: Don't stick with *"everything is a file"*.**
>
> In UNIX like systems, everything is a file, which means that you can access *almost* everything, including devices, as if they were files. This is a powerful concept I like, but I intentionally avoid it in FTL.
>
> The reason is that file is not always the best abstraction. Plan 9 do very well with this concept, in most UNIX like systems, instead of file reads/writes, we need to `ioctl` to do something special. Plan 9 did it right, but in FTL, I'd avoid spending time on making everything file-ish, but provide a simple and clear interface. Bi-drectional message passing is still simple but is more expressive than file.

## List of Kernel Objects

FTL provides following kernel objects:

- **Process**: A running program instance.
- **Thread**: An execution unit of a process.
- **Vmspace**: A virtual memory space shared by multiple processes.
- **Folio**: A contiguous memory region.
- **Channel**: A bi-directional, asynchronous, and bounded message queue.
- **Signal**: A lightweight notification mechanism.
- **Poll**: A polling mechanism for asynchronous I/O.
- **Interrupt**: A hardware interrupt.

## Handle

Each kernel object is reference counted. When you create a kernel object, you get an opaque integer, called a *handle*. It is the similar concept to *file descriptors* in Linux.

### Capability

Each handle does not directly point to the kernel object. Instead, it's a tuple of `(object, rights)`, where `rights` is a set of allowed operations on the object. This is called *capability*.

### Handle Table

In FTL, system calls can be considered as methods to use kernel objects. Each process has a *handle table* that maps handles to kernel objects. System calls take a handle as an argument, look up in the handle table, and operate on the kernel object.

Thus, threads in a process share the same kernel objects because they have the same handle table.

### Reference Counting

Each kernel object (not handle!) is reference counted. Closing a handle is equivalent to decrementing the reference count. When the reference count reaches 0, the kernel object is destroyed.

What actually happens when an object is destroyed depends on the object type.
