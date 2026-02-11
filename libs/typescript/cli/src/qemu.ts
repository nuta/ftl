export interface QemuParams {
    inheritStdin?: boolean;
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
        "-netdev", "user,id=net0,hostfwd=tcp:127.0.0.1:30080-:80",
        "-device", "virtio-net-pci,netdev=net0",
        "-object", "filter-dump,id=filter0,netdev=net0,file=network.pcap"
    ];

    const stdin = params.inheritStdin ? "inherit" : null;
    return Bun.spawn(["qemu-system-x86_64", ...args], { stdio: [stdin, "inherit", "inherit"] });
}
