
## Kernel Objects

```rust
struct Process {
    page_table: PageTable,
    threads: LinkedList<Thread>,
    handles: [Handle; 256],
    indirect_handles: [HandleTable; 16],
}

struct RiscvPageTableL0;
struct RiscvPageTableL1;
struct MemoryPool;
struct DataPage;

struct Thread {
    state: ThreadState,
    context: Context,
    sched: Scheduling,
    process: ParentRef<Process>,
}

struct Channel;

struct IrqPool;
struct Irq;
```

- `RandControl`
  - `Rand`
- `DebugControl`
  - `PutChar`
- `TimeControl`
  - `Uptime`
  - `Walltime`


# FTL

## Motivation

What if we build a new microkernel-based general-purpose operating system with modern concepts in mind? FTL is a research project that aims to answer this question.

## Kernel

FTL Kernel is a microkernel which provides memory management, process/thread management, interrupt delivery, and IPC. The design principle is to provide the minimum set of features that really need to be in the kernel. For example, FTL kernel does not provide any file system, device driver, or network stack. Instead, these features are implemented in userland processes called *servers*. This design allows us to implement these features in a more flexible way without kernel programming.

FTL kernel is object-based and capability-based. Kernel resources such as page frames and process are represented as *kernel objects*. Userland programs access kernel objects through *handles*, a pair of the reference to a kernel object and a set of permissions. When userland programs invoke a system call, they specify the *handle ID*, an integer to identify a handle in the process. The kernel then looks up the handle table of the current process and checks if the handle ID is valid and the requested operation is permitted for the handle. Handle can be duplicated, partially revoked, and transferred between processes. This mechanism allows fine-grained access control and resource sharing in userland.

System calls are as follows:


## Servers

FTL servers are userland processes that provide operating system features such as file system, device driver, and network stack. You can consider servers as a replacement for Linux kernel modules, or programs managed by systemd.

