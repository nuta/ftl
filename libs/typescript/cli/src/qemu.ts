import fs from "fs/promises";

export interface QemuParams {
    inheritStdin?: boolean;
}

export async function startQemu(params: QemuParams) {
    const scsiDiskPath = "/tmp/ftl-virtio-scsi-disk.img";
    const scsiDiskSize = 64 * 1024 * 1024;

    const scsiDisk = await fs.open(scsiDiskPath, "a");
    await scsiDisk.truncate(scsiDiskSize);
    await scsiDisk.close();

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
        "-device", "virtio-scsi-pci,id=scsi0",
        "-drive", `if=none,id=scsidisk0,file=${scsiDiskPath},format=raw`,
        "-device", "scsi-hd,drive=scsidisk0,bus=scsi0.0"
    ];

    const stdin = params.inheritStdin ? "inherit" : null;
    return Bun.spawn(["qemu-system-x86_64", ...args], { stdio: [stdin, "inherit", "inherit"] });
}
