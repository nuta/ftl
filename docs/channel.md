# Channel

## System calls

### `channel_send(ch, msginfo, msgbuffer) -> ()`

Sends a message. The message will be delivered to `ch`'s peer. It copies the `msginfo.len`-bytes message data and moves `msginfo.handles` handles to the peer channel queue.

If the kernel fails returns an error code, it's guaranteed that `handles` will not be transferred. That is, **the caller must keep ownership of the handles**. This is why `Channel::send` returns `SendError` instead of `FtlError`.

### `channel_recv(ch, msgbuffer) -> msginfo` **(blocking)**

Waits for and receives a message. Message will be copied to `msgbuffer`.

# Layout

```plain

|63 or 31                                      14|13   12|11         0|
+------------------------------------------------+-------+------------+
|                        ID                      |   H   |    LEN     |
+------------------------------------------------+-------+------------+

LEN  (12 bits) - # of bytes in the message data.
H    (2 bits)  - # of handles in the message.
ID (rest)    - Message ID.

```

# Design decisions

## A single `isize` for everything (`ID`, `H`, and `LEN`)

- It can fit in a single CPU register. Specifying the message metadata
  can be done with simply setting an immediate value to a register.

- The message structure (i.e. its length and the number of handles) can be
  validated at once when checking the message ID.

- It forcibly limits the message data length and the number of handles, in
  other words, the kernel just need to do bitwise AND operations to get them,
  without any additional checks.

## Message metadata is not part of the message buffer

- If it was part of the message buffer, the kernel would need to read the
  message buffer first to determine the number/length of handles/data that
  it needs to copy.

- It's useful for debugging. We can determine the message ID even if an
  app accidentally passed an invalid pointer to the kernel. It could be a
  key clue for debugging.

## The maximum message length is 4095 bytes (0xfff)

- Big enough for inlined file read and ethernet frame. It’s convenient for prototyping.
- Small enough to keep it allocated, even on the stack. It allows reusing the same memory space for all messages, like L4's virtual/message registers.
- Not big enough for bulk send. It encourages you to use transferring Buffer handle instead of memory copies via kernel.
- Ideally it should be 4KiB to be page-sized, but 4096 (0x1000) will require two steps to read the message length: bitwise AND (0x1fff) and then validate if it's less than 0x1000.

## A pointer to a single constant-sized buffer, not separate pointers to `data` and `handles`

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

## `handles` in the message buffer come after the data part

- If `handles` come before the data part, when copying the data part, we need to calculate the offset of the data part. It would be done in only one arithmetic instruction, but I'd optimize it as much as possible.
