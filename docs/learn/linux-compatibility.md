# Linux Compatibility

> [!CAUTION]
> **Not Yet Implemented:** The feasibility of this feature is confirmed in PoC, but it's not yet implemented. Stay tuned.

Linux compatibility is an upcoming feature to enable incremental adoption of FTL. While it is not merged yet, we'd make it clear what we're going to do.

## ABI-based vs. VM-based Compatibility

There are 2 main approaches to achieve Linux compatibility: ABI-based and VM-based.

### ABI-based Compatibility

ABI-based compatibility is about implementing Linux ABI, especially Linux system calls. This allows seamless integration with the OS, and the overhead is smaller than the VM-based, but requires a lot of work to make it fully compatible.

[FreeBSD](https://docs.freebsd.org/en/books/handbook/linuxemu/) and [Fuchsia](https://fuchsia.dev/fuchsia-src/concepts/starnix) are famous examples of this approach.

### VM-based Compatibility

This approach is straightforward: run a real Linux in a virtual machine. This obviously provides full Linux compatibility, and the overhead is higher than the ABI-based. What's worse, it requires hardware acceleration to make it performant. That is, you need bare-metal instances in the cloud to run efficiently, not cheap VM-based instances because nested virtualization is slow.

However, it's much easier to implement and maintain because the hypervisor interface (virtio devices) won't change dramatically. The host OS does not have to care about what's happening inside the VM like io_uring. Using hypervisors doesn't always mean it's slow. For example, [My VM is Lighter (and Safer) than your Container (Filipe Manco, et al. SOSP '17)](https://dl.acm.org/doi/10.1145/3132747.3132763) showed the hypervisor overhead can be minimized.

[Windows Subsystem for Linux 2 (WSL)](https://learn.microsoft.com/en-us/windows/wsl/about) is a good example of this approach.

## Planned Approach

FTL plans to implement ABI-based compatibility to achieve a well-isolated Linux  environment without bare-metal instances, like gVisor.
