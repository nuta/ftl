import path from "node:path";
import { buildQemuArgs } from "../tests/helpers/vm";

const TIMEOUT_MS = 10 * 1000;
const ROOT = path.join(import.meta.dir, "..");

interface Benchmark {
  iterations: number;
}

const BENCHMARKS: Record<string, Benchmark> = {
  getpid: { iterations: 1000 },
  pipe: { iterations: 1000 },
  "file-read": { iterations: 1000 },
  mmap: { iterations: 100 },
};

async function run(name: string, { iterations }: Benchmark): Promise<number> {
  const qemu = Bun.spawn(
    buildQemuArgs({
      init: `/bin/util bench ${name} ${iterations}`,
      // Make the QEMU's RTC deterministic:
      //
      // shift=0: 1 nanosecond per instruction
      // sleep=off: On HLT, fire the next timer interrupt immediately
      extra: ["-icount", "shift=0,sleep=off"],
    }),
    { cwd: ROOT, stdin: "ignore", stdout: "pipe", stderr: "ignore" },
  );

  const killTimer = setTimeout(() => qemu.kill(), TIMEOUT_MS);

  // Extract the elapsed time from the output.
  let output = "";
  const decoder = new TextDecoder();
  for await (const chunk of qemu.stdout) {
    output += decoder.decode(chunk, { stream: true });
    const m = /elapsed_ns=(\d+)\r?\n/.exec(output);
    if (m) {
      clearTimeout(killTimer);
      qemu.kill();

      // 1ns = 1 instruction thanks to -icount shift=0.
      return Math.floor(Number(m[1]) / iterations);
    }
  }

  clearTimeout(killTimer);
  throw new Error(`${name}: failed or timed out\n${output}`);
}

async function main() {
  const names = Object.keys(BENCHMARKS).sort();
  for (const name of names) {
    const insns = await run(name, BENCHMARKS[name]);
    console.log(`${name.padEnd(12)} ${String(insns).padStart(10)} insns/iter`);
  }
}

await main();
