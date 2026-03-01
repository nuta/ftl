import EventEmitter from "node:events";
import { AddressInfo } from "node:net";
import * as net from "node:net";
import { QemuParams, startQemu } from "../../libs/typescript/cli/src/qemu";

async function getRandomPort(): Promise<number> {
    const socket = new net.Server();
    return new Promise((resolve, reject) => {
        socket.on('error', reject);
        socket.on('listening', () => {
            const port = (socket.address() as AddressInfo).port;
            socket.close();
            resolve(port);
        });

        socket.listen({ port: 0, host: '127.0.0.1' });
    });
}

async function streamToLines(stream: ReadableStream<Uint8Array>, cb: (line: string) => void) {
    const decoder = new TextDecoder();
    const reader = stream.getReader();

    let buf = "";
    while (true) {
        const { done, value } = await reader.read();
        if (done) {
            return;
        }

        buf += decoder.decode(value, { stream: true });
        const lines = buf.split(/\n|\r\n?/);
        buf = lines.pop() ?? "";
        for (const line of lines) {
            cb(line);
        }
    }
}

function createDebouncer<T>(callback: (...args: T[]) => void, ms: number) {
    let timeout: NodeJS.Timeout | null = null;
    return (...args: T[]) => {
        if (timeout) {
            clearTimeout(timeout);
        }

        timeout = setTimeout(() => {
            callback(...args);
            timeout = null;
        }, ms);
    };
}

export type BootParams = Omit<QemuParams, 'stdio'> & {
};

export async function boot(params: BootParams) {
    const child = await startQemu({ ...params, stdio: ['pipe', 'pipe', 'pipe'] });

    const emitter = new EventEmitter();

    if (!(child.stdout instanceof ReadableStream)) {
        throw new Error(`stdout is not a readable stream: ${typeof child.stdout}`);
    }

    if (!(child.stderr instanceof ReadableStream)) {
        throw new Error(`stderr is not a readable stream: ${typeof child.stderr}`);
    }

    const emitLog = createDebouncer((line: string) => {
        emitter.emit('log');
    }, 1);

    const logs: string[] = [];
    streamToLines(child.stdout, (line) => {
        logs.push(line);
        emitLog();
    }).catch((error) => {
        console.error(`stdout stream error: ${error}`);
    });

    streamToLines(child.stderr, (line) => {
        logs.push(line);
        emitLog();
    }).catch((error) => {
        console.error(`stderr stream error: ${error}`);
    });

    child.exited.then(() => {
        emitter.emit('exited', child.exitCode);
    });

    const waitForLogs = async (callback: (logs: string[]) => void) => {
        return new Promise<void>((resolve, reject) => {
            let lastError: unknown = null;

            let timeout: NodeJS.Timeout | null = null;
            const rearmTimeout = () => {
                if (timeout) {
                    clearTimeout(timeout);
                }

                timeout = setTimeout(() => {
                    reject(`waitForLogs timed out: ${lastError}`);
                }, 1000);
            };

            rearmTimeout();
            emitter.on('log', async () => {
                try {
                    await callback(logs);
                    resolve();
                } catch (error) {
                    // Assertion failures. Try again later.
                    lastError = error;
                } finally {
                    rearmTimeout();
                }
            });

            emitter.on('exited', async (exitCode) => {
                reject(new Error(`QEMU exited with ${exitCode}: ${logs.join('\n')}`));
            });
        });
    };

    return {
        waitForLogs,
        [Symbol.dispose]() {
            child.kill();
        }
    }
}
