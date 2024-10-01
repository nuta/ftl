---
title: Concepts
---

FTL kernel is a collection of kernel objects, such as processes, threads, channels, and memory spaces. Let's dive into the concepts of FTL to understand its design and philosophy.

> [!TIP]
>
> **Design decision: Do not stick with *"everything is a file"*.**
>
> In UNIX like systems, everything is a file, which means that you can access *almost* everything, including devices, as if they were files. This is a powerful concept I like, but I intentionally avoid it in FTL.
>
> The reason is that file is not always the best abstraction. Plan 9 do very well with this concept, in most UNIX like systems, instead of file reads/writes, we need to `ioctl` to do something special. Plan 9 did it right, but in FTL, I'd avoid spending time on making everything file-ish, but provide a simple and clear interface. Bi-drectional message passing is still simple but is more expressive than file.
