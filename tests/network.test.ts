import { expect, test } from "bun:test";

import { boot, getAvailablePort } from "./helpers/vm";

test("HTTP server handles a request", async () => {
    const hostPort = await getAvailablePort();
    using vm = await boot({ hostPort, init: "/bin/httpd" });
    await vm.waitForLog("HTTP server listening on port 80");

    const resp1 = await fetch(`http://127.0.0.1:${hostPort}`, {
        signal: AbortSignal.timeout(500),
    });
    expect(resp1.status).toBe(200);
    expect(await resp1.text()).toContain('<h1 class="logo">FTL</h1>');

    const resp2 = await fetch(`http://127.0.0.1:${hostPort}/missing`, {
        signal: AbortSignal.timeout(500),
    });
    expect(resp2.status).toBe(404);
    expect(await resp2.text()).toContain("<h1>404 Not Found</h1>");
}, 5000);

test("TCP active open", async () => {
    using server = Bun.listen({
        hostname: "127.0.0.1",
        port: 0,
        socket: {
            data(socket, data) {
                socket.end(`echo: ${data}`);
            },
        },
    });

    const hostPort = await getAvailablePort();
    using vm = await boot({ hostPort, init: `/bin/util nc 10.0.2.2 ${server.port}` });
    await vm.waitForLog("echo: hello from nc");
}, 5000);

test("UDP send and receive", async () => {
    const server = await Bun.udpSocket({
        hostname: "127.0.0.1",
        port: 0,
        socket: {
            data(socket, data, port, address) {
                socket.send(`echo: ${data}`, port, address);
            },
        },
    });

    try {
        const hostPort = await getAvailablePort();
        using vm = await boot({ hostPort, init: `/bin/util nc -u 10.0.2.2 ${server.port}` });
        await vm.waitForLog("echo: hello from nc");
    } finally {
        server.close();
    }
}, 5000);
