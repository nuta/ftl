import { startQemu } from "../qemu";
import * as build from "./build";

export async function main(args: string[]) {
    await build.main([]);

    const qemu = await startQemu({
        inheritStdin: true
    });
    await qemu.exited;
    if (qemu.exitCode !== 0) {
        throw new Error(`QEMU failed with ${qemu.exitCode}`);
    }
}
