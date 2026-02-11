import { build, BuildParams } from "../build";

export async function main(args: string[]) {
    const params: BuildParams = {
        mode: process.env.BUILD === 'release' ? 'release' : 'debug',
        arch: 'x64',
        apps: ['virtio_scsi', 'tcpip', 'http_server'],
    }

    await build(params);
}
