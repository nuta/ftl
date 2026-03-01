import { createInitfs } from "./initfs";
import { Arch } from "./types";
import * as fs from 'node:fs/promises';

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

    await runCmd(argv, {
        env: { ...process.env, CARGO_TERM_COLOR: 'always', CARGO_TERM_HYPERLINKS: 'false' },
    });
}

export async function runCmd(argv: string[], options?: Parameters<typeof Bun.spawn>[1]) {
    const proc = Bun.spawn(argv, {
        ...options,
        stdio: [
            'inherit',
            'inherit',
            'inherit',
        ],
    });

    await proc.exited;
    if (proc.exitCode !== 0) {
        throw new Error(`command failed with ${proc.exitCode}`);
    }
}

export async function build(params: BuildParams) {
    const targetJSON = `libs/rust/ftl/src/arch/${params.arch}/user.json`;
    const cargoArgs = [
        '-Z', 'build-std=core,alloc',
        '-Z', 'build-std-features=compiler-builtins-mem',
    ];

    await cargoBuild('apps/bootstrap/Cargo.toml', targetJSON, params.mode, cargoArgs);
    await runCmd([
        'llvm-objcopy',
        '-Obinary',
        '--set-section-flags',
        '.bss=alloc,load,contents',
        `target/user/${params.mode}/bootstrap`,
        'bootstrap.bin',
    ]);

    const initfsFiles: Record<string, string> = {};
    for (const app of params.apps) {
        const manifestPath = `apps/${app}/Cargo.toml`;
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
