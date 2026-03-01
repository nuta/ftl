export type QemuStdio = "inherit" | "pipe" | null;

export interface PortForward {
    protocol: "tcp";
    hostPort: number;
    guestPort: number;
}

export interface QemuParams {
    portForwarding?: PortForward[];
    stdio: [QemuStdio, QemuStdio, QemuStdio];
}

export async function startQemu(params: QemuParams) {
    const args = [
        "-m", "128",
        "-cpu", "qemu64,+fsgsbase",
        "-kernel", "ftl.elf",
        "-initrd", "initfs.tar",
        "-nographic",
        "-serial", "mon:stdio",
        "--no-reboot",
        "-gdb", "tcp::7778",
        "-d", "cpu_reset,unimp,guest_errors,int",
        "-D", "qemu.log",
        "-device", "virtio-net-pci,netdev=net0",
        "-object", "filter-dump,id=filter0,netdev=net0,file=network.pcap"
    ];

    const hostfwds = []
    for (const { protocol, hostPort, guestPort } of params.portForwarding ?? []) {
        hostfwds.push(`${protocol}:127.0.0.1:${hostPort}-:${guestPort}`);
    }

    if (hostfwds.length > 0) {
        args.push("-netdev", `user,id=net0,hostfwd=${hostfwds.join(',')}`);
    } else {
        args.push("-netdev", "user,id=net0");
    }

    return Bun.spawn(["qemu-system-x86_64", ...args], { stdio: params.stdio });
}
