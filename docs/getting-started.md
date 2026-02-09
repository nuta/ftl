# Getting Started

## Install Packages

Install the following packages:

- [Rustup](https://rustup.rs/)
- [Bun](https://bun.com/docs/installation)
- QEMU (`brew install qemu` or `apt install qemu`)

> [!TIP]
>
> **Can I use Windows?**
>
> Yes, it should just work on Windows. That said, I recommend using WSL2 (Ubuntu) for good performance and ease of setup.

## Running FTL

To build and run the OS, run:

```bash
bin/ftl run
```

> [!NOTE]
>
> To exit QEMU, type <kbd>Ctrl+A</kbd> then <kbd>X</kbd>. Or <kbd>C</kbd> to enter QEMU monitor (debug console).

## Auto Restarting

To automatically rebuild and restart the OS when you make changes, run:

```bash
bin/ftl dev
```

> [!NOTE]
>
> To exit, type <kbd>Ctrl+C</kbd>.
