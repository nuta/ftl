# Process and Thread

Process and thread are the same concepts as you know from other operating systems.

## Process

Process is a collection of threads and handles.

### Handle Table

A process has a handle table, which is a `HashMap<HandleId, Handle>` where `HandleId` is an integer, and `Handle` is a kernel object (e.g. a channel) and allowed operations on it (so-called capabilities). A handle is similar to a file descriptor in Linux.

When a thread calls a system call, it often passes a handle ID to reference a kernel object. The kernel's system call implementation looks up the handle ID in the handle table to get the corresponding handle, and does the job.

## Isolation

A process *belongs to* (not *owns*) an isolation, which defines how memory safety is enforced and provides a memory address space for the belonging processes.

Unlike Linux where each process has its own page table, multiple processes can share the same page table (isolation). This unlocks unique features like unikernel-style isolation, where multiple processes can share the same kernel's address space safely thanks to Rust's memory safety guarantees.

See [Isolation](./isolation) for more details.

## Thread

Thread is a unit of execution. A program starts with a single thread, running from the `main` function sequentially.

You can create multiple threads to run something in parallel. Those threads will start from the entry point you specified.

All threads in a process share the process' resources. That is:

- Memory address space. The same global variables are accessible to all threads.
- Handles. [Channels](./channel) for example.
