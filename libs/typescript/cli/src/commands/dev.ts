import { startQemu } from "../qemu";
import * as buildCommand from "./build";
import fs from 'fs/promises';
import path from 'path';

const SOURCE_EXTENSIONS = new Set([
    '.rs',
    '.toml',
    '.ts',
    '.js',
    '.json',
    '.html',
]);

async function debounce(ms: number, fn: () => Promise<void>): Promise<void> {
    let timeout: NodeJS.Timeout | null = null;
    return new Promise((resolve, reject) => {
        if (timeout) {
            clearTimeout(timeout);
        }

        timeout = setTimeout(() => {
            fn().then(resolve).catch(reject);
        }, ms);
    });
}

export async function main(args: string[]) {
    let qemu: ReturnType<typeof Bun.spawn> | null = null;

    process.on('exit', () => {
        if (qemu) {
            qemu.kill('SIGTERM');
        }
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
            qemu = await startQemu();
        } catch (error) {
            console.error(`failed to run: ${error}`);
        }
    };

    const watchDir = path.resolve(import.meta.dir, '..', '..', '..', '..', '..');
    console.log(`Watching for changes in ${watchDir}...`);
    await rebuild();
    const watcher = fs.watch(watchDir, { recursive: true });
    for await (const { eventType, filename } of watcher) {
        if (!filename || filename.startsWith('build/') || filename.startsWith('target/')) {
            continue;
        }

        if (!SOURCE_EXTENSIONS.has(path.extname(filename))) {
            continue;
        }

        await debounce(5, async () => {
            await rebuild(filename);
        });
    }
}
