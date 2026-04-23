import { createInitfs } from "./initfs";
import { Arch } from "./types";
import * as fs from 'node:fs/promises';
import * as path from 'node:path';

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

async function runCmdWithOutput(argv: string[]): Promise<string> {
    const proc = Bun.spawn(argv, {
        stdout: 'pipe',
        stderr: 'pipe',
    });

    const stdout = new Response(proc.stdout).text();
    const stderr = new Response(proc.stderr).text();
    await proc.exited;
    const stdoutText = await stdout;

    if (proc.exitCode !== 0) {
        const stderrText = await stderr;
        throw new Error(`command failed with ${proc.exitCode}: ${argv.join(' ')}\n${stderrText.trim()}`);
    }

    return stdoutText.trim();
}

async function pathExists(filePath: string): Promise<boolean> {
    try {
        await fs.access(filePath);
        return true;
    } catch {
        return false;
    }
}

async function findRustLlvmTool(tool: string): Promise<string> {
    const sysroot = await runCmdWithOutput(['rustc', '--print', 'sysroot']);
    const rustlib = path.join(sysroot, 'lib', 'rustlib');
    const entries = await fs.readdir(rustlib, { withFileTypes: true });

    for (const entry of entries) {
        if (!entry.isDirectory()) {
            continue;
        }

        const candidate = path.join(rustlib, entry.name, 'bin', tool);
        if (await pathExists(candidate)) {
            return candidate;
        }
    }

    throw new Error(`could not find ${tool} in ${rustlib}; install Rust's llvm-tools component with: rustup component add llvm-tools`);
}

export async function build(params: BuildParams) {
    const targetJSON = `libs/rust/ftl/src/arch/${params.arch}/user.json`;
    const cargoArgs = [
        '-Z', 'build-std=core,alloc',
        '-Z', 'build-std-features=compiler-builtins-mem',
    ];

    await cargoBuild('apps/bootstrap/Cargo.toml', targetJSON, params.mode, cargoArgs);
    const llvmObjcopy = await findRustLlvmTool('llvm-objcopy');
    await runCmd([
        llvmObjcopy,
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
