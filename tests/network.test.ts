import { expect, test } from "bun:test";

import { boot, getAvailablePort } from "./helpers/vm";

test("HTTP server handles a request", async () => {
    const hostPort = await getAvailablePort();
    using vm = await boot(hostPort);
    await vm.waitForLog("HTTP server listening on port 80");

    const resp1 = await fetch(`http://127.0.0.1:${hostPort}`, {
        signal: AbortSignal.timeout(500),
    });
    expect(resp1.status).toBe(200);
    expect(await resp1.text()).toContain("<h1>FTL operating system</h1>");

    const resp2 = await fetch(`http://127.0.0.1:${hostPort}/missing`, {
        signal: AbortSignal.timeout(500),
    });
    expect(resp2.status).toBe(404);
    expect(await resp2.text()).toContain("<h1>404 Not Found</h1>");
}, 5000);
