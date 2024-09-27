---
title: System Calls
---

## Notation

In this document, system call interfaces are defined in C. This is because Rust interface are high-level and not directly correspond to the actual ABI.

Here are some common types used in the interfaces:

```c
#include <stdint.h>

/// `isize` in Rust. 32-bits or 64-bits depending on CPU architecture.
typedef intmax_t isize_t;

/// Error code type. All error codes are negative.
typedef isize_t error_t;

/// Handle ID.
typedef isize_t handle_t;

/// Handle rights.
typedef uint8_t handle_rights_t;

/// Message information.
typedef int32_t msginfo_t;

/// Signal value.
typedef int32_t signalval_t;

/// Poll event.
typedef int32_t pollevent_t;

/// IRQ number.
typedef int32_t irq_t;

/// The user-space address. It's signed to reserve the negative values for
/// errors. Practically it's OK because the upper half of the address space
/// is reserved for the kernel.
typedef isize_t uaddr_t;

/// Poll result. This single isize_t packs a handle ID (handle_t)
/// and a poll event (pollevent_t).
typedef isize_t pollresult_t;

/// Failable operation return values.
///
/// If the value is negative, it's an error code.
typedef isize_t handle_or_error_t;
typedef isize_t msginfo_or_error_t;
typedef isize_t signalval_or_error_t;
typedef isize_t pollresult_or_error_t;
typedef isize_t uaddr_or_error_t;
```

> [!TIP]
>
> **Design principle**: Negative values are reserved for error codes. System
> calls should return a positive value on success, and that's why types are
> signed integers.

## Debug Console I/O

*"Debug console"* is a device which the kernel uses to output debug messages. Typically, it is a UART device, aka. serial port.

### `console_write`

Writes a buffer to the debug console. Used for debugging purposes.

```c
error_t console_write(const char *buf, isize_t len);
```

## Handle Management

### `handle_close`

Closes a handle. It's an equivalent of `close` system call in Linux.

```c
error_t handle_close(handle_t handle);
```

- This decrements the reference count of the object associated with the handle. If it's still referenced by other handles, the object is not destroyed.
- It depends on the object type how the kernel handles the close operation.

### `handle_restrict` (NOT IMPLEMENTED)

Drops the capability of the handle.

```c
error_t handle_restrict(handle_t handle, handle_rights_t rights);
```

- This system call is used to restrict the capability of the handle. For example, if the handle is a channel, it can only send messages but not receive.


### `handle_clone` (NOT IMPLEMENTED)

Duplicates a handle.

```c
handle_or_error_t handle_clone(handle_t handle);
```

- `handle` should be `HANDLE_CLONEABLE` (NOT IMPLEMENTED).

## Channel

Channel is a bi-directional and asynchronous message queue used for inter-process communication (IPC). Each channel is connected to another channel (called *peer*).

### `channel_create`

Creates a channel pair connected to each other.

```c
handle_or_error_t channel_create(void);
```

Due to the system call limitation that it can only return one value, the handle IDs of the created channel pair need to be computed in the following way:

```c
handle_or_error_t handle1 = channel_create();
handle_t handle2 = handle1 + 1;
```

That is, the second handle ID is always the first handle ID plus one.

### `channel_send`

Sends a message to the channel's peer.

```c
error_t channel_send(handle_t ch, msginfo_t msginfo, const void *msgbuffer);
```

TODO: Describe the message format.

- The message will be delivered to the channel's *peer*, not the specified channel.
- This operation is non-blocking and asynchronous. If the peer's message queue is full, it immediately returns an error code.

### `channel_recv`

Waits for a message to arrive on the channel.

```c
msginfo_or_error_t channel_recv(handle_t ch, void *msgbuffer);
```

TODO: Describe the message format.

- This operation is blocking. If there is no message in the queue, it waits until a message arrives.

### `channel_try_recv`

Receives a message if there is one in the channel. If there is no message, it returns an error code immediately (non-blocking).

```c
msginfo_or_error_t channel_try_recv(handle_t ch, void *msgbuffer);
```

## Signal

Signal is a mechanism to notify a process that a particular event has occurred. It's similar to (but not equivalent to) *semaphore*.

Each signal has a 32-bit integer bitfield. It will be updated by `signal_update`, and will be read by `signal_clear`.

Unlike channels, signals are unidirectional and it cannot tell how many times and who updated the signal.

### `signal_create`

Creates a signal.

```c
handle_or_error_t signal_create(void);
```

### `signal_update`

Updates the signal's value. Once the signal is updated, any waiting process will be woken up.

```c
error_t signal_update(handle_t signal, signalval_t value);
```

### `signal_clear`

Clears the signal's value, and returns the previous value.

```c
signalval_or_error_t signal_clear(handle_t signal);
```

## Poll

Poll is a mechanism to wait for multiple events to occur. It's similar to `epoll` system call in Linux.

This is the key underlying mechanism for `Mainloop` in Rust API.

### `poll_create`

Creates a poll object.

```c
handle_or_error_t poll_create(void);
```

### `poll_add`

Adds a kernel object to the poll.

```c
error_t poll_add(handle_t poll, handle_t object, pollevent_t interests);
```

`interests` is a bitfield of events to wait for. It can be a combination of the following values:

- `READ`: Wait for the object to be readable. For example, a channel has a message to receive.
- `WRITE`: Wait for the object to be writable. Not implemented as of this writing.

### `poll_remove`

Removes a kernel object from the poll.

```c
error_t poll_remove(handle_t poll, handle_t object);
```

### `poll_wait`

Waits for an event to occur on the poll.

```c
pollresult_or_error_t poll_wait(handle_t poll);
```

The return value is a packed value of a handle ID of the object and a poll event that occurred.

## Folio

Folio represents an ownership of a memory block. The memory block is
page-aligned (typically 4KB), and contiguous in the physical address space.

### `folio_create`

Allocates a memory block at an arbitrary physical address.

```c
handle_or_error_t folio_create(usize_t len);
```

- Folio is not accessible by the user-space directly. It needs to be mapped
  into the virtual memory space first by `vmspace_map`.

### `folio_create_fixed`

Allocates a memory block at a fixed physical address.

```c
handle_or_error_t folio_create_fixed(usize_t len, uaddr_t paddr);
```

- If the physical address is already used, it fails and returns an error code.


### `folio_paddr`

Returns the physical address of the folio.

```c
uaddr_or_error_t folio_paddr(handle_t folio);
```

## Vmspace

Vmspace represents a virtual memory space of a process. Unlike Linux, where each process has its own address space, FTL processes can share the same address space.

### `vmspace_create` (NOT IMPLEMENTED)

Creates a new virtual memory space.

```c
handle_or_error_t vmspace_create(void);
```

### `vmspace_map`

Maps a folio into the virtual memory space, and returns the mapped address.

```c
uaddr_or_error_t vmspace_map(handle_t vmspace, usize_t len, handle_t folio, prot_t prot);
```

- The address will be assigned by the kernel. Consider it as random.

### `vmspace_unmap` (NOT IMPLEMENTED)

Unmaps a folio from the virtual memory space.

```c
error_t vmspace_unmap(handle_t vmspace, uaddr_t addr);
```

- `addr` must be the starting address of the folio.

## Interrupt

Interrupt is an ownership of a hardware interrupt line to implement device
drivers.

### `interrupt_create`

Creates an interrupt object and enables the interrupt line.

```c
handle_or_error_t interrupt_create(irq_t irq);
```

- If the interrupt line is already used, it fails and returns an error code.

### `interrupt_ack`

Acknowledges the interrupt.

```c
error_t interrupt_ack(handle_t interrupt);
```

- This system call is used to tell the kernel that the interrupt is handled and
  you are ready for the next interrupt. Unless you call this system call, the
  interrupts on the line would remain pending.

## Process

Process is a set of threads sharing a virtual memory space (vmspace) and
handles.

### `process_create` (NOT IMPLEMENTED)

Creates a new process.

```c
handle_or_error_t process_create(handle_t vmspace, const char *name);
```

### `process_inject_handle` (NOT IMPLEMENTED)

Moves a handle to the process.

```c
handle_or_error_t process_inject_handle(handle_t process, handle_t handle);
```

- Once the handle is injected, the handle is no longer valid in the caller
  process. It's moved to the target process.
- This system call returns the handle ID in the target process.

### `process_exit`

Exits the current process.

```c
void process_exit(void);
```

- This system call never fails and never returns.

## Thread

### `thread_create` (NOT IMPLEMENTED)

Creates a new thread in the current process.

```c
handle_or_error_t thread_create(handle_t process, const char *name);
```

TODO: Should it return the handle ID in caller process or the target process? I think the former is better, but what if we want both?

### `thread_start` (NOT IMPLEMENTED)

Starts the thread from the entry point (`entry`) with an argument (`arg`).

```c
error_t thread_start(handle_t thread, uaddr_t entry, uaddr_t arg);
```

### `thread_exit` (NOT IMPLEMENTED)

Exits the current thread.

```c
void thread_exit(void);
```
