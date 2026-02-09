import { createInitfs } from "./initfs";
import { Arch } from "./types";
import fs from 'fs/promises';

export interface BuildParams {
    mode: 'debug' | 'release';
    arch: Arch;
    apps: string[];
}

async function cargoBuild(manifestPath: string, targetJSON: string, mode: 'debug' | 'release', args: string[] = []): Promise<void> {
    let argv = [
        'cargo',
        'build',
        '-Z', 'build-std=core,alloc',
        '-Z', 'build-std-features=compiler-builtins-mem',
        '--manifest-path', manifestPath,
        '--target', targetJSON,
        ...args,
    ]

    if (mode === 'release') {
        argv.push('--release');
    }

    const proc = Bun.spawn(argv, {
        env: {
            ...process.env,
            CARGO_TERM_COLOR: 'always',
            CARGO_TERM_HYPERLINKS: 'false',
        },
        stdio: [
            'inherit',
            'inherit',
            'inherit',
        ],
    });

    await proc.exited;
    if (proc.exitCode !== 0) {
        throw new Error(`build failed with ${proc.exitCode}`);
    }
}

export async function build(params: BuildParams) {
    const cargoArgs = [
        '-Z', 'build-std=core,alloc',
        '-Z', 'build-std-features=compiler-builtins-mem',
    ];

    const initfsFiles: Record<string, string> = {};
    for (const app of params.apps) {
        const manifestPath = `apps/${app}/Cargo.toml`;
        const targetJSON = `libs/rust/ftl/src/arch/${params.arch}/user.json`;
        await cargoBuild(manifestPath, targetJSON, params.mode, cargoArgs);
        initfsFiles[app] = `target/user/debug/${app}`;
    }

    const kernelManifestPath = 'kernel/Cargo.toml';
    const kernelTargetJSON = `kernel/src/arch/${params.arch}/kernel.json`;
    await cargoBuild(kernelManifestPath, kernelTargetJSON, params.mode, cargoArgs);

    const initfs = await createInitfs(initfsFiles);
    await Bun.write('initfs.tar', initfs);

    await fs.copyFile(`target/kernel/${params.mode}/kernel`, 'ftl.elf');
}
