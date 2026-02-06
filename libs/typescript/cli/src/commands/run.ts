import * as build from "./build";

export async function run(args: string[]) {
    await build.run([]);

    const qemuArgs = [
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

    const qemu = Bun.spawn(["qemu-system-x86_64", ...qemuArgs], { stdio: ["inherit", "inherit", "inherit"] });
    await qemu.exited;
    if (qemu.exitCode !== 0) {
        throw new Error(`QEMU failed with ${qemu.exitCode}`);
    }
}
