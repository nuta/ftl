---
title: Writing Your First Server
---

## Scaffolding

## Define an interface in IDL

## `Mainloop` API

## Accepting a new client

## Per-client state

## Reply to the client

## Design patterns

### Track all incoming messages in the main loop

`ChannelSender` to send to clients from multiple places.

### `main.rs` for all message handling

The main logic of your server should be in a separate file.

```rust
let server = YourServerImpl::new();
loop {
    match mainloop.next() {
        ... => {
            server.handle_message_a(...);
        }
        ... => {
            server.handle_message_b(|channel_sender| {
                channel_sender.send(...);
            });
        }
    }
}
```

## Do not trust the client!

## Error handling

## Next Steps
