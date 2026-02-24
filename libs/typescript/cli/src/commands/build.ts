import { build, BuildParams } from "../build";

export async function main(args: string[]) {
    const params: BuildParams = {
        mode: process.env.BUILD === 'release' ? 'release' : 'debug',
        arch: 'x64',
        apps: ['linux_compat'],
    }

    await build(params);
}
