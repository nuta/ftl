# Kernel

## Channel

## System calls

### `channel_send(ch, header, buf, handles) -> ()`

Sends a message. The message will be delivered to `ch`'s peer.

If the kernel fails returns an error code, it's guaranteed that `handles` will not be transferred. That is, **the caller must keep ownership of the handles**. This is why `Channel::send` returns `SendError` instead of `FtlError`.
