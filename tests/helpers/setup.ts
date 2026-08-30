import path from "node:path";

const build = Bun.spawn([path.join(__dirname, "..", "..", "build.sh")], {
    cwd: path.join(__dirname, "..", ".."),
    stdout: "inherit",
    stderr: "inherit",
});

if (await build.exited !== 0) {
    throw new Error("build failed");
}
