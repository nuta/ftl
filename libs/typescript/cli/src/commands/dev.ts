import { startQemu } from "../qemu";
import * as buildCommand from "./build";
import * as fs from 'node:fs/promises';
import * as path from 'node:path';

const SOURCE_EXTENSIONS = new Set([
    '.rs',
    '.toml',
    '.ts',
    '.js',
    '.json',
    '.html',
]);

const QEMU_START_RETRY_DELAYS_MS = [100, 250, 500, 1000, 2000, 5000, 10000];
const QEMU_START_SETTLE_MS = 250;
const QEMU_STOP_TIMEOUT_MS = 1000;

type QemuProcess = ReturnType<typeof Bun.spawn>;

function sleep(ms: number) {
    return new Promise((resolve) => setTimeout(resolve, ms));
}

function createDebouncer(ms: number, fn: (filename?: string) => Promise<void>) {
    let timeout: NodeJS.Timeout | null = null;
    let promise = Promise.resolve();

    return (filename: string) => {
        if (timeout) {
            clearTimeout(timeout);
        }

        timeout = setTimeout(() => {
            promise = promise
                .then(() => fn(filename))
                .catch((error) => {
                    console.error(`failed to run: ${error}`);
                })
                .finally(() => {
                    timeout = null;
                });
        }, ms);
    };
}

async function stopQemu(qemu: QemuProcess) {
    qemu.kill('SIGTERM');

    const exited = await Promise.race([
        qemu.exited.then(() => true),
        sleep(QEMU_STOP_TIMEOUT_MS).then(() => false),
    ]);

    if (exited) {
        return;
    }

    qemu.kill('SIGKILL');
    await qemu.exited;
}

async function startQemuWithRetry(params: Parameters<typeof startQemu>[0]) {
    for (let attempt = 0; attempt <= QEMU_START_RETRY_DELAYS_MS.length; attempt++) {
        const qemu = await startQemu(params);
        const stillRunning = await Promise.race([
            qemu.exited.then(() => false),
            sleep(QEMU_START_SETTLE_MS).then(() => true),
        ]);

        if (stillRunning) {
            return qemu;
        }

        const delay = QEMU_START_RETRY_DELAYS_MS[attempt];
        if (delay === undefined) {
            throw new Error(`QEMU exited immediately with ${qemu.exitCode}`);
        }

        console.log(`QEMU exited immediately; retrying in ${delay}ms...`);
        await sleep(delay);
    }

    throw new Error('QEMU failed to start');
}

export async function main(args: string[]) {
    let qemu: QemuProcess | null = null;

    process.on('exit', () => {
        if (qemu) {
            qemu.kill('SIGTERM');
        }
    });

    process.on('SIGINT', async () => {
        console.log('exiting...');
        if (qemu) {
            const oldQemu = qemu;
            qemu = null;
            await stopQemu(oldQemu);
        }

        process.exit(0);
    });

    const rebuild = async (filename?: string) => {
        if (qemu) {
            const oldQemu = qemu;
            qemu = null;
            await stopQemu(oldQemu);
        }

        // Clear the screen.
        if (process.stdout.isTTY) {
            console.log('\x1b[2J\x1b[H');
        }

        if (filename) {
            console.log(`Changed: ${filename}`);
        }

        try {
            await buildCommand.main([]);
            qemu = await startQemuWithRetry({
                portForwarding: [
                    // HTTP server
                    {
                        protocol: "tcp",
                        hostPort: 30080,
                        guestPort: 80,
                    },
                ],
                // Do not inherit stdin, so that Ctrl-C will exit QEMU
                stdio: [null, 'inherit', 'inherit'],
            });
        } catch (error) {
            console.error(`failed to run: ${error}`);
        }
    };

    const watchDir = path.resolve(import.meta.dir, '..', '..', '..', '..', '..');
    console.log(`Watching for changes in ${watchDir}...`);
    await rebuild();
    const scheduleRebuild = createDebouncer(50, rebuild);
    const watcher = fs.watch(watchDir, { recursive: true });
    for await (const { eventType, filename } of watcher) {
        if (!filename || filename.startsWith('build/') || filename.startsWith('target/')) {
            continue;
        }

        if (!SOURCE_EXTENSIONS.has(path.extname(filename))) {
            continue;
        }

        scheduleRebuild(filename);
    }
}
