---
title: Memory Management
---

In FTL, kernel allocates memory and manages virtual memory spaces. This document explains key concepts of memory management in FTL.

## Folio

Folio is the ownership of a *physically* contiguous page-aliged memory region. Are you faimilar with memory pages? Folio is a multiple of contiguous memory pages.

Folio is used for the following purposes:

- Allocating memory for applications.
- Shared memory between applications.
- Acquiring MMIO regions from device drivers.

Owning a folio does not mean you can access the memory directly. You need to map it into your virtual memory space, which is explained in the next section.

## Vmspace

Vmspace (Virtual Memory Space) represents a virtual memory space of processes. Essentially, it's a collection of  areas, each of which is mapped to a folio.

> [!TIP]
>
> **Design decision: Kernel allocates memory.**
>
> This decision sounds obvious, but it's not. In modern microkernels, such as seL4, the kernel only provides memory management primitives, and the userland is responsible for allocating memory. This principle is called [separation of mechanism and policy](https://en.wikipedia.org/wiki/Separation_of_mechanism_and_policy).
>
> FTL took a somewhat legacy approach: the kernel still implements the memory allocation *policy*. While this is not ideal, it allows intuitive APIs and simplifies the kernel implementation. In the future, we may consider moving the memory allocator to userland (or in-kernel application), but it's not a priority as of now.
