
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
    process: Process,
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
