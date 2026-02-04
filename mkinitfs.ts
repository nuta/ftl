const archive = new Bun.Archive({
    // "hello.elf": await Bun.file("target/user/debug/hello").arrayBuffer(),
    // "ping.elf": await Bun.file("target/user/debug/ping").arrayBuffer(),
    // "pong.elf": await Bun.file("target/user/debug/pong").arrayBuffer(),
    "virtio_net": await Bun.file("target/user/debug/virtio_net").arrayBuffer(),
    "tcpip": await Bun.file("target/user/debug/tcpip").arrayBuffer(),
    "http_server": await Bun.file("target/user/debug/http_server").arrayBuffer(),
})

await Bun.write("initfs.tar", archive);
