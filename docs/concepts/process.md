---
title: Process
---

Like popular operating systems, each running program instance (or application) in FTL is represented as a *process*.

Process can be considered as a container of other kernel objects such as:

- Virtual memory space (`vmspace` object).
- System resources (`handle` object).
- Threads of execution (`thread` object).

## Virtual Memory Space

In Linux, each process has its own address space. In FTL, on the other hand, it's slightly different - each process belongs to a *virtual memory space*. This means that multiple processes can share the same address space! It enables to implement a lightweight IPC mechanism without copies nor kernele involvement (not implemented yet though).

## Handles

Each process has a set of *handles* to access kernel objects. You might be familiar with the concept of file descriptors in Linux. Like you specify a file descriptor to access a file in Linux, you specify a handle to access a kernel object in FTL.

## Threads

Threads are execution units sharing the same resources, that is, a process. Thread is not something special at all in FTL, but it's more lightweight than other operating systems.

