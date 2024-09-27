---
title: Channel
---

Channel is a bi-directional, asynchronous, and bounded message queue used for inter-process communication (IPC).

## Interface Definition Language (IDL)

In FTL, message definitions are written in the Interface Definition Language (IDL), which is actually JSON/YAML. `ftl_autogen` generates the message definitions in Rust so that IPC can be language-agnotic and type-safe.

See [IDL](../spec/interface.md) for more details.

## Backpressure

Channel is a bounded message queue, which means that it has a limited capacity. When the queue is full, the sender will fail to send a message. This mechanism is called *backpressure*.

Backpressure may happen in the following cases:

- The receiver is too slow to consume messages and cannot keep up with the sender's speed.
- The receiver hung up and is not consuming messages anymore.

You should always consider the send operation failure and handle it properly. That said, how should we handle the failure? It depends on the application, but here are some common strategies:

| Strategy | When to use | Remarks |
| --- | --- | --- |
| Drop the message | Server can tolerate message loss | Network packets between device drivers and TCP/IP server. TCP can handle packet loss. |
| Drop the message and close the channel | Replying to clients from a server | Close the channel so that clients can notice the situation. |
| Wait and retry | The message is important and should not be lost. | File system writes, and writes to the TCP sockets in TCP/IP server. |

> [!TIP]
>
> **Backpressure is a rate limiting mechanism.**
>
> The capacity of the channel is the maximum length of data clients may request. Like Linux's pipe has [a size limit](https://man7.org/linux/man-pages/man7/pipe.7.html#:~:text=to%20a%20pipe.-,Pipe%20capacity,-A%20pipe%20has), you should always be aware of the capacity.
>
> Interestingly, the channel capacity can be used as a rate limiting mechanism. For example, connection channel created for each TCP connection can be used as a TCP buffer, and listen channel can be used as TCP backlog.


> [!TIP]
>
> **Design decision: Do not stick with *"everything is a file"*.**
>
> In UNIX like systems, everything is a file, which means that you can access *almost* everything, including devices, as if they were files. This is a powerful concept I like, but I intentionally avoid it in FTL.
>
> The reason is that file is not always the best abstraction. Plan 9 do very well with this concept, in most UNIX like systems, instead of file reads/writes, we need to `ioctl` to do something special. Plan 9 did it right, but in FTL, I'd avoid spending time on making everything file-ish, but provide a simple and clear interface. Bi-drectional message passing is still simple but is more expressive than file.

> [!TIP]
>
> **Design decision: Do not do everything in message passing.**
>
> In other words, FTL should provide multiple ways for IPC, designed for different purposes. I plan to add another message-passing like mechanism for bulk data transfers for performance critical services such as network/file system stacks.
