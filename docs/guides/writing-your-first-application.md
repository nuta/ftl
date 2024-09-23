---
title: Writing Your First Application
---

> [!NOTE]
>
> See [Quickstart](../quickstart.md) first to set up the development environment.

Let's write a simple Hello World application. In this section, we'll create a simple app called `demo`.

## Scaffolding

The first step is to generate a template for your application. FTL provides a handy tool for you at `tools/scaffold.py`. Just run it with the `--type app <NAME>` option:

```
$ ./tools/scaffold.py --type app demo
  GEN apps/demo/Cargo.toml
  GEN apps/demo/build.rs
  GEN apps/demo/main.rs
  GEN apps/demo/app.spec.json

==> generated app at apps/demo
```

Now you have a new directory at `apps/demo` with the following files:

- `Cargo.toml`: The Cargo manifest file (see [The Cargo Book](https://doc.rust-lang.org/cargo/reference/manifest.html)).
- `build.rs`: The build script file (see [The Cargo Book](https://doc.rust-lang.org/cargo/reference/build-scripts.html)).
- `main.rs`: The main source file.
- `app.spec.json`: The FTL application manifest file.

## Running the application

To run the application, execute `make run APPS=apps/<name>`. `APPS` is a space-separated list of applications to run. You will see the following log message from the `demo` app:

```
$ make run APPS=apps/demo
...
[demo        ] INFO   Hello World!
```

## Discover FTL APIs

FTL API (`ftl_api`) provides a set of useful functions. For example:

```rust
use ftl_api::prelude::*; // info! and Vec

// Print a message to the debug console.
let answer = 42;
info!("answer is {}", answer);

// A variable-length array (so-called vector or list).
let mut vec = Vec::new();
vec.push(1);
vec.push(2);
vec.push(3);
info!("vec: {:?}", vec);

// HashMap (so-called dictionary or associative array).
use ftl_api::collections::HashMap;
let mut map = HashMap::new();
map.insert("apple", 1000);
map.insert("banana", 2000);
map.insert("cherry", 3000);
info!("map: {:?}", map);
```

Discover more `no_std` APIs in [crates.io](https://crates.io/categories/no-std?sort=downloads) to focus on what you actually want to implement!

## Connect with services

In Linux and other major operating systems, applications calls system calls to use OS services such as file systems, TCP/IP networking, device drivers, etc. In microkernel architecture, we still use system calls, but the actual OS services are provided by separate userspace programs connected over inter-process communication (IPC).

In FTL, each service (or *server*) provides a set of APIs over a message-passing mechanism called *channel*. Channel is a bi-directional, asynchronous message queue between two processes.

Here, let's connect the `demo` application with `apps/ping_server` app, which is a simple server which replies a simple message, just like `ping` command in Linux.

However, how can we know the server's channel? In FTL, the service dependencies are managed through systemd/Kubernetes-like declarative configuration files called *spec files*. Declare a new dependency in `app.spec.json`:

```json
{
  "name": "demo",
  "kind": "app/v0",
  "spec": {
    "depends": [
      {
        "name": "ping_server",
        "type": "service",
        "protocol": "ping"
      }
    ],
    "provides": []
  }
}
```

Now, FTL will automatically connect the `ping_server` service. You can get the channel via `Environ`, the first parameter of the `main` function:

```rust
#![no_std]
#![no_main]

use ftl_api::environ::Environ;
use ftl_api::prelude::*;

#[no_mangle]
pub fn main(mut env: Environ) {
    info!("env: {:#?}", env);
    let ping_server_ch = env.take_channel("dep:ping_server").unwrap();
    info!("ping_server_ch: {:?}", ping_server_ch);
}
```

Run the application with `ping_server`:

```
$ make run APPS="apps/demo apps/ping_server"
...
[demo        ] INFO   env: {
    "dep:startup": Channel(
        Channel(#2),
    ),
    "dep:ping_server": Channel(
        Channel(#1),
    ),
}
[demo        ] INFO   ping_server_ch: Channel(#1)
```

You can see the `ping_server` channel is connected to the `demo` application!

> [!TIP]
>
> **What is `dep:startup`?**
>
> You may notice that there is another channel named `dep:startup`. This is a channel which is connected to the service which started the application.
>
> You will see more about this in [Writing Your First Server](writing-your-first-server) guide.

## Interface Defniition Language (IDL)

We are almost there! Now, we have a channel to the `ping_server` service. However, how can we know what kind of messages we can send to the server? In FTL, we use Interface Definition Language (IDL) to define the message format.

You can find the IDL file at `idl.json`. Here is the IDL for the `ping_server` service (`ping`):

```json
{
  "name": "ping",
  "rpcs": [
    {
      "name": "ping",
      "request": {
        "fields": [
          {
            "name": "value",
            "type": "int32"
          }
        ]
      },
      "response": {
        "fields": [
          {
            "name": "value",
            "type": "int32"
          }
        ]
      }
    }
  ]
},
```

There's a RPC (send-then-receive operation like HTTP) called `"ping"`. Both request/resuponse messages have a single 32-bit integer field `value`. This is what we'll try!

Now we know the service protocol, you might wonder how to define the message structure in Rust. No worries! FTL will auto-generate the message structure for you in `build.rs` using `    ftl_autogen::generate_for_app`, which `scaffold.py` has already done.

To import the generated code, add the following line to `main.rs`:

```rust
ftl_api::autogen!();

// Import the generated code.
use ftl_autogen::idl::ping::Ping;
use ftl_autogen::idl::ping::PingReply;
```

This internally calls [`include!`](https://doc.rust-lang.org/std/macro.include.html) macro to include the generated code. The auto generated code will be embedded into the file directly, as `ftl_autogen` module.

> [!TIP]
>
> **Why not defniing interfaces in Rust?**
>
> Rust `struct`s with procedural macros are powerful, but we prefer IDL because:
>
> - IDL is language-agnostic. We plan to support other programming languages in the future.
> - It's easier to debug and maintain the auto-generated code.
> - JSON is easier to read and write by programs. For example, we don't need to port Rust compiler to build a web-based IDL visualizer.

## Send a message to the server

You're now ready to send a message to the `ping_server` service! Let's send and receive a message:

```rust
#![no_std]
#![no_main]

// Embed the auto-generated code from IDL.
ftl_api::autogen!();

use ftl_api::environ::Environ;
use ftl_api::prelude::*;
use ftl_api::types::message::MessageBuffer;

// Use the auto-generated message definitions.
use ftl_autogen::idl::ping::Ping;
use ftl_autogen::idl::ping::PingReply;

#[no_mangle]
pub fn main(mut env: Environ) {
    // Get the channel to the ping_server.
    let ping_server_ch = env.take_channel("dep:ping_server").unwrap();
    info!("ping_server_ch: {:?}", ping_server_ch);

    // Prepare a memory buffer to receive a message.
    let mut msgbuffer = MessageBuffer::new();
    loop {
        // Send a message to the server asynchronously.
        ping_server_ch.send(Ping { value: 10 }).unwrap();

        // Wait for a reply from the server.
        let reply = ping_server_ch
            .recv_with_buffer::<PingReply>(&mut msgbuffer)
            .unwrap();

        // We've got a reply successfully!
        info!("got a reply: {:?}", reply);
    }
}
```

Run the application with `ping_server`. You will see infinite log messages like this:

```
$ make run APPS="apps/demo apps/ping_server"
...
[demo        ] INFO   ping_server_ch: Channel(#1)
[ping_server ] INFO   ping_server started
[demo        ] INFO   got a reply: PingReply { value: 0 }
[demo        ] INFO   got a reply: PingReply { value: 1 }
[demo        ] INFO   got a reply: PingReply { value: 2 }
[demo        ] INFO   got a reply: PingReply { value: 3 }
[demo        ] INFO   got a reply: PingReply { value: 4 }
[demo        ] INFO   got a reply: PingReply { value: 5 }
[demo        ] INFO   got a reply: PingReply { value: 6 }
[demo        ] INFO   got a reply: PingReply { value: 7 }
[demo        ] INFO   got a reply: PingReply { value: 8 }
[demo        ] INFO   got a reply: PingReply { value: 9 }
```

It works! You've successfully written your first FTL app!

## Next steps

Interestingly, this guide covers most of what you need to know to write an FTL application. You will need to learn few more APIs to write OS services, but the basic concepts are the same: scaffold your app with `tools/scaffold.py`, fill the spec file to inject dependencies into `Environ`, and communicate with other components over channels.

[Writing Your First Server](writing-your-first-server.md) is a good next step to learn how to write an OS service.
