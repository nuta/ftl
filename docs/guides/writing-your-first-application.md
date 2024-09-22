---
title: Writing your first application
---

Let's write a simple Hello World application. In this section, we'll create a simple app called `demo`.

## Scaffolding

The first step is to generate a template for your application. FTL provides a handy tool for you at `tools/scaffold.py`. Just run it with the `--type app <NAME>` option:

```
$ ./tools/scaffold.py --type app demo
  GEN apps/demo/Cargo.toml
  GEN apps/demo/main.rs
  GEN apps/demo/app.spec.json

==> generated app at apps/demo
```

Now you have a new directory at `apps/demo` with the following files:

- `Cargo.toml`: The Cargo manifest file (see [The Cargo Book](https://doc.rust-lang.org/cargo/reference/manifest.html)).
- `main.rs`: The main source file. You can write your application here.
- `app.spec.json`: The application manifest file.

## Running the application

To run the application, execute `make run APPS=apps/<name>`:

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

// HashMap (so-called map, dictionary, or associative array).
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

However, how can we know the server's channel? In FTL, the service dependencies are managed through systemd/Kubernetes-like declarative configuration files called *spec files*. Add a new entry in `depends`:

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

Now, FTL will automatically connect the `ping_server` service! You can get the channel via `Environ`, the first parameter of the `main` function:

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

> [!TIP] **What's `dep:startup`?**
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

> [!TIP] **Why not defniing interfaces in Rust?**
>
> Rust `struct`s with procedural macros are powerful, but they are not suitable for IPC. However, we prefer IDL because:
>
> - IDL is language-agnostic. We plan to support other programming languages in the future.
> - It's easier to debug and maintain the auto-generated code.

## Next steps

[Writing Your First Serverrver](writing-your-first-server) is a good next step to learn how to write an OS service.
