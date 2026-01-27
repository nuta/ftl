const archive = new Bun.Archive({
    "hello.elf": await Bun.file("target/user/debug/hello").arrayBuffer(),
})

await Bun.write("initfs.tar", archive);
