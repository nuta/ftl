import { build, BuildParams } from "../build";

export async function run(args: string[]) {
    const params: BuildParams = {
        arch: 'x64',
        apps: ['virtio_net', 'tcpip', 'http_server'],
    }

    await build(params);
}
