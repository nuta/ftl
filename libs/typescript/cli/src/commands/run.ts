import { startQemu } from "../qemu";
import * as build from "./build";

export async function main(args: string[]) {
    await build.main([]);

    const qemu = await startQemu({
        portForwarding: [
            // HTTP server
            {
                protocol: "tcp",
                hostPort: 30080,
                guestPort: 80,
            },
        ],
        stdio: ['inherit', 'inherit', 'inherit'],
    });
    await qemu.exited;
    if (qemu.exitCode !== 0) {
        throw new Error(`QEMU failed with ${qemu.exitCode}`);
    }
}
