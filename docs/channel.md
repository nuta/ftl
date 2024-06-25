# Channel

Channel is an asynchronous, bounded, and bi-directional message-passing mechanism between processes.

## Why classic two-copies message passing?

In FTL, to implement the nature of "async", the kernel will copy the message into the kernel memory, and copy it again to the receiver process' memory.

If you're familar with recent microkernels design, you might find this channel design a bit old-fashioned. For example, IPC in L4 microkernels is synchronous to avoid having a in-kernel queue, and they avoid expensive memory copies by forcing much smaller message size to even fit in CPU registers. Futhermore, some recent microkernels use shared memory + asynchronous notification mechanism to avoid memory copies and keep the kernel minimal.

Why FTL did not follow these modern designs? In fact, I've tried to design a shared memory-based IPC mechanism, which can be used for RPC, message passing, byte streams like TCP, and [`tokio::sync::watch`](https://docs.rs/tokio/latest/tokio/sync/watch/index.html)-like primitive. It was promising. However, I couldn't come up with an intuitive API/implementation, the most important FTL's design principle, was not satisfied. Making more memory copies allow easier to use and understand interface!. `.clone()`-ing is much easier than managing lifetime parameters, right?

I'd stress that I do care about performance, but in most cases we can start with a simple API like Linux's `read(2)`. If you hit the performance bottleneck, then you can optimize it later with a better interface like `io_uring`. Also, thanks to intutive and simplified design of the channel API, we might be able to introduce a new channel implementaion in the kernel transparenly!

## System calls

### `channel_send(ch, msginfo, msgbuffer) -> ()`

Sends a message. The message will be delivered to `ch`'s peer. It copies the `msginfo.len`-bytes message data and moves `msginfo.handles` handles to the peer channel queue.

If the kernel fails returns an error code, it's guaranteed that `handles` will not be transferred. That is, **the caller must keep ownership of the handles**. This is why `Channel::send` returns `SendError` instead of `FtlError`.

### `channel_recv(ch, msgbuffer) -> msginfo` **(blocking)**

Waits for and receives a message. Message will be copied to `msgbuffer`.

If the queue is empty, it will block until a message is available.

## Message Layout

### Message Buffer

```rust
#[repr(C, align(16))] // Align to 16 bytes for SIMD instructions.
pub struct MessageBuffer {
    pub data: [u8; MESSAGE_DATA_MAX_LEN /* 4095 (0b1111_1111_1111) */],
    pub handles: [HandleId; MESSAGE_HANDLES_MAX_COUNT /* 3 (0b11) */],
}
```

### Message Info

```plain

|63 or 31                                      14|13   12|11         0|
+------------------------------------------------+-------+------------+
|                        ID                      |   H   |    LEN     |
+------------------------------------------------+-------+------------+

LEN  (12 bits) - # of bytes in the message data.
H    (2 bits)  - # of handles in the message.
ID (rest)    - Message ID.

```

## Remarks

### Message queue is bounded (not-yet-implemented)

The channel queue is bounded (roughly 64KB by default). If the queue is full, the kernel returns an error when sending a message immediately (i.e. non-blocking). This prevents a malicious (or buggy) app from consuming all the memory, or spamming (aka. denial-of-service attack) a server process.

The behavior when the queue is full is open to discussion. Here are some options:

- Abort the program with an error message.
- Retry sending the message later.
- Discard the message if replying to a client from a server process or at-most-once delivery is acceptable (e.g. TCP packet from the network; the remote peer will retransmit again).

## Design decisions

### A single `isize` for everything (`ID`, `H`, and `LEN`)

- It can fit in a single CPU register. Specifying the message metadata
  can be done with simply setting an immediate value to a register.

- The message structure (i.e. its length and the number of handles) can be
  validated at once when checking the message ID.

- It forcibly limits the message data length and the number of handles, in
  other words, the kernel just need to do bitwise AND operations to get them,
  without any additional checks.

### Message metadata is not part of the message buffer

- If it was part of the message buffer, the kernel would need to read the
  message buffer first to determine the number/length of handles/data that
  it needs to copy.

- It's useful for debugging. We can determine the message ID even if an
  app accidentally passed an invalid pointer to the kernel. It could be a
  key clue for debugging.

### The maximum message length is 4095 bytes (0xfff)

- Big enough for inlined file read and ethernet frame. It’s convenient for prototyping.
- Small enough to keep it allocated, even on the stack. It allows reusing the same memory space for all messages, like L4's virtual/message registers.
- Not big enough for bulk send. It encourages you to use transferring Buffer handle instead of memory copies via kernel.
- Ideally it should be 4KiB to be page-sized, but 4096 (0x1000) will require two steps to read the message length: bitwise AND (0x1fff) and then validate if it's less than 0x1000.

### A pointer to a single constant-sized buffer, not separate pointers to `data` and `handles`

Let's compare with an alternative interface design for "send a message" API. Here's the signature of `zx_channel_write` syscall in Fuchsia ([doc](https://fuchsia.dev/reference/syscalls/channel_write)):

```c
zx_status_t zx_channel_write(zx_handle_t handle,
                             uint32_t options,
                             const void* bytes,
                             uint32_t num_bytes,
                             const zx_handle_t* handles,
                             uint32_t num_handles);
```

It's super intuitive, right? In contrast, FTL merges `bytes` and `handles` into a single constant-sized buffer called *"message buffer"*. This is because:

- It makes FTL developers (you!) aware that they need to prepare a big enough buffer for receiving theoritically maximum-sized message. Otherwise, malicious clients can easily cause a buffer overflow by sending a huge message.
- Fewer system call parameters. Fewer instructions and memory accesses when sending/receiving messages. I don't think this matters much by the way.

### `handles` in the message buffer come after the data part

- If `handles` come before the data part, when copying the data part, we need to calculate the offset of the data part. It would be done in only one arithmetic instruction, but I'd optimize it as much as possible.
