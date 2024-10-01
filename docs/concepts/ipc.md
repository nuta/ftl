---
title: IPC (Inter-Process Communication)
---

FTL provides multiple ways for inter-process communication (IPC). The most basic and fundamental way is message passing, which is implemented as a *channel* in FTL.

> [!TIP]
>
> **Design decision: Do not do everything in message passing.**
>
> In other words, FTL should provide multiple ways for IPC, designed for different purposes. I plan to add another message-passing like mechanism for bulk data transfers for performance critical services such as network/file system stacks.

## Channel

Channel is a bi-directional, asynchronous, and bounded message queue used for inter-process communication (IPC).

### Interface Definition Language (IDL)

In FTL, message definitions are written in the Interface Definition Language (IDL), which is actually JSON/YAML. `ftl_autogen` generates the message definitions in Rust so that IPC can be language-agnotic and type-safe.

TODO: Link

### Backpressure

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
