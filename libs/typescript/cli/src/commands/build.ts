import { build, BuildParams } from "../build";

export async function main(args: string[]) {
    const params: BuildParams = {
        mode: process.env.BUILD === 'release' ? 'release' : 'debug',
        arch: 'x64',
        // apps: ['virtio_net', 'tcpip', 'http_server', 'ping', 'pong'],
        apps: ['virtio_net', 'tcpip', 'ping', 'pong'],
    }

    await build(params);
}
