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

export async function main(args: string[]) {
    let qemu: ReturnType<typeof Bun.spawn> | null = null;

    process.on('exit', () => {
        if (qemu) {
            qemu.kill('SIGTERM');
        }
    });

    process.on('SIGINT', () => {
        console.log('exiting...');
        if (qemu) {
            qemu.kill('SIGTERM');
        }

        process.exit(0);
    });

    const rebuild = async (filename?: string) => {
        if (qemu) {
            qemu.kill('SIGTERM');
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
            if (qemu) {
                await qemu.exited;
            }
            qemu = await startQemu({
                enableGDB: true,
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
