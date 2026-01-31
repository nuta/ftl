const archive = new Bun.Archive({
    "ping.elf": await Bun.file("target/user/debug/ping").arrayBuffer(),
    "pong.elf": await Bun.file("target/user/debug/pong").arrayBuffer(),
})

await Bun.write("initfs.tar", archive);
