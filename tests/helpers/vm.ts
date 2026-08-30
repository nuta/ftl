import path from "node:path";
import * as net from "node:net";

export async function getAvailablePort(): Promise<number> {
    const server = net.createServer();
    await new Promise<void>((resolve, reject) => {
        server.once("error", reject);
        server.listen(0, "127.0.0.1", resolve);
    });

    const address = server.address();
    if (!address || typeof address === "string") {
        throw new Error("failed to allocate a port");
    }

    await new Promise<void>((resolve) => server.close(() => resolve()));
    return address.port;
}

function buildQemuArgs({ hostPort }: { hostPort: number }) {
    return [
        "qemu-system-x86_64",
        "-m", "128",
        "-cpu", "qemu64,+fsgsbase,+xsave,+xsaveopt",
        "-kernel", "ftl.elf",
        "-initrd", "initfs.cpio",
        "-nographic",
        "-serial", "mon:stdio",
        "--no-reboot",
        "-device", "isa-debug-exit,iobase=0x501,iosize=0x04",
        "-netdev", `user,id=net0,hostfwd=tcp:127.0.0.1:${hostPort}-:80`,
        "-device", "virtio-net-pci,netdev=net0",
    ]
}

export async function boot(hostPort: number) {
    const qemu = Bun.spawn(buildQemuArgs({ hostPort }), {
        cwd: path.join(__dirname, "..", ".."),
        stdin: "ignore",
        stdout: "pipe",
        stderr: "pipe",
    });

    let output = "";
    const readStream = async (stream: ReadableStream<Uint8Array>) => {
        for await (const chunk of stream) {
            output += new TextDecoder().decode(chunk);
        }
    };

    readStream(qemu.stdout);
    readStream(qemu.stderr);

    return {
        async waitForLog(text: string) {
            while (!output.includes(text)) {
                if (qemu.exitCode !== null) {
                    throw new Error(`QEMU exited with ${qemu.exitCode}:\n${output}`);
                }

                await Bun.sleep(200); // FIXME: emit event instead of polling
            }
        },
        [Symbol.dispose]() {
            qemu.kill();
        },
    };
}
