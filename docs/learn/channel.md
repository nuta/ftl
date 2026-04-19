# Channel


Channel is an Inter-Process Communication (IPC) mechanism in FTL.

## Overview

Channel is:

- A bounded message queue, where each message contains a byte array and a handle array.
- Connected to its peer channel and is bi-directional. Messages are delivered to the peer channel.
- Asynchronous and non-blocking.
- Movable between processes.

## Message

A message is a unit of transfer between channels. It is a packet-like structure that contains:

| Field | Type | Description |
|-------|------|-----------|
| Message info | `u32` | Message type, ID, and body length. |
| Inlined arguments | `(usize, usize)` | Arbitrary data that is copied as is. |
| Message body | `&mut [u8]` | A pair of pointer and length of a memory region to be copied to the peer process. |
| Handle | `Handle` | A handle (e.g. channel) to be moved to the peer process. |

## Message Types

In applications, channel is used a RPC mechanism between processes: opening a TCP socket, reading/writing data, etc.

RPC is built on top of channel. Messages are categorized into request and reply, like `open` and `open_reply`. Here's the complete list of request/reply message types:

```rs
// Opens a resource. Equivalent to: open(2), socket(2), bind(2), listen(2)
fn open(path: &[u8], options: OpenOptions) -> Channel

// Reads data. Equivalent to: pread(2)
fn read(offset: usize, len: usize) -> Vec<u8>

// Writes data. Equivalent to: pwrite(2)
fn write(offset: usize, buf: &[u8]) -> usize /* written bytes */

// Gets an attribute. Equivalent to: stat(2)
fn getattr(attr: Attr) -> Vec<u8>

// Sets an attribute. Equivalent to: chmod(2), rename(2), ....
fn setattr(attr: Attr, buf: &[u8]) -> usize /* written bytes */
```

Any request can be replied with an error (`error_reply` message).

> [!NOTE]
> **Design decision: Schema-less IPC**
>
> Unlike other IPC systems like gRPC, Fuchsia's FIDL, or Mach's MIG, FTL does not use Interface Definition Language (IDL) to define message types.
>
> This approach sacrifices some type safety and flexibility. However, IDL requires auto-generated code (IPC stubs) which introduces another layer of abstraction to learn, and adds tons of verbose code into the Git repository.
>
> My finding is that OS service interfaces are generally simple, compared to applications. By providing a limited set of message types, OS services have similar IPC patterns, which makes it easier to understand and optimize.

> [!NOTE]
>
> **Design decision: Prefer pull-based communication**
>
> You may notice that the client process asks the server process for data explicitly (pull-based),
> instead of the server process simply sends a data message to the client (push-based).
>
> This approach liberates the kernel from allocating the buffer for the message body,
> and naturally encourages backpressure.

## Channel vs. `std::sync::mpsc`

[`std::sync::mpsc`](https://doc.rust-lang.org/std/sync/mpsc/fn.channel.html) (`mpsc` for short) is a similar API in Rust's standard library. Both our channel and `mpsc` channel are message queues, but they are different in some aspects.

### Channel can transfer handles

`mpsc` channel is for inner-process communication. FTL channel is for *inter*-process communication, which supports transferring handles between processes.

### Bi-directional vs. Uni-directional

`mpsc` channel creates a pair of sender and receiver (uni-directional):

```rust
let (tx, rx) = std::sync::mpsc::channel();
```

Our channel creates a pair of channels connected to each other (bi-directional):

```rust
let (ch1, ch2) = ftl::channel::Channel::new().unwrap();
```

Both `ch1` and `ch2` can send/receive messages. Sending a message using `ch1` will deliver the message to `ch2`, and vice versa. This is similar to TCP connections where both sides can send/receive data.

## Internals

Under the hood, FTL channel is implemented by following system calls (simplified):

```rs
impl Channel {
    fn create() -> (Channel, Channel);
    fn send(&self, msg: Message);
    fn recv(&self, mid: MessageId, buf: &mut [u8]) -> HandleId;
    fn discard(&self, info: MessageInfo);
}
```

The notable difference from typical IPC systems is the concept of *"peek"*. The receive operation is non-blocking, and receives a specific message selected by its ID, not any message.

> ![NOTE]
>
> **Design decision: Peek then receive**
>
> To efficiently handle messages, you first peek at the message in the queue to examine its type, ID, and other metadata (e.g. the body length). This allows you to decide how to handle each message before actually reading it.
>
> This design eliminates the need to allocate an extra intermediate buffer for the incoming data, and applications can naturally implement backpressure (i.e. pause the sender) until you receive the message.

For example, in server side (i.e. accept any messages), the typical operation is to peek then receive the message:

```rs
fn server_main(ch: &Channel) -> Result<(), ErrorCode> {
    // Watch events from the channel.
    let mut sink = Sink::new()?;
    sink.add(ch)?;

    // Wait for a message.
    match sink.wait()? {
        // Message is ready. The message is automatically peeked.
        Event::Message(Peek { info, .. }) => {
            let mid = info.mid();
            match info.kind() {
                MessageKind::WRITE => {
                    let mut buf = vec![0; info.body_len()];

                    // Receive the message.
                    ch.recv_body(info, &mut buf)?;

                    info!("received a write message: {:?}", core::str::from_utf8(&buf));
                    ch.send(Message::WriteReply { mid, len: buf.len() })?;
                }
                _ => {
                    ch.send(Message::ErrorReply { mid, error: ErrorCode::Unsupported })?;
                }
            }
        }
        // The peer closed the channel, and no more messages can be received.
        Event::PeerClosed => return Err(ErrorCode::PeerClosed),
    }

    Ok(())
}
```

In client side (i.e. receive a specific message) is a RPC-like operation: send a request and wait for a reply. This looks very similar to the server side, and the key difference is that the client side expects a specific message kind:

```rs
fn client_main(ch: &Channel) -> Result<(), ErrorCode> {
    let mid = MessageId::new(1);
    ch.send(Message::Write { mid, offset: 0, buf: b"Hello, world!" });

    // Watch events from the channel.
    let mut sink = Sink::new()?;
    sink.add(&ch)?;

    // Wait for a message.
    let (id, event) = sink.wait()?;
    match event {
        Event::Message(Peek { info, arg1, .. }) if info.mid() == mid => {
            match info.kind() {
                MessageKind::WRITE_REPLY => {
                    // We already know the written length through the peek, but
                    // need to consume the reply message.
                    ch.recv_args(info)?;

                    info!("received a write reply: written_len={}", arg1);
                }
                MessageKind::ERROR_REPLY => {
                    // We already know the error code through the peek, but
                    // need to consume the reply message.
                    ch.recv_args(info)?;

                    panic!("server replied with an error: {:?}", arg1);
                }
                _ => {
                    panic!("unexpected message type: {:?}", info.kind());
                }
            }
        }
        Event::Message(Peek { info, .. }) => {
            // The server replied to an unknown request.
            panic!("unexpected mid: {:?}", info.mid());
        }
        Event::PeerClosed => {
            // The server has died. The client should retry or cleanup.
            panic!("peer has closed the channel");
        }
    }
}
```

> [!NOTE]
>
> Receving a message is a non-blocking operation. If there is no message that matches the message info, it returns immediately with an error. Sink is the only way to wait for events including channel messages.
