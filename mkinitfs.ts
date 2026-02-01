const archive = new Bun.Archive({
    // "hello.elf": await Bun.file("target/user/debug/hello").arrayBuffer(),
    "ping.elf": await Bun.file("target/user/debug/ping").arrayBuffer(),
    "pong.elf": await Bun.file("target/user/debug/pong").arrayBuffer(),
})

await Bun.write("initfs.tar", archive);
