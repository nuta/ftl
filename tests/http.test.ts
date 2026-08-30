import { expect, test } from "bun:test";

import { boot, getAvailablePort } from "./helpers/vm";

test("HTTP server handles a request", async () => {
    const hostPort = await getAvailablePort();
    using vm = await boot(hostPort);
    await vm.waitForLog("HTTP server listening on port 80");

    const response = await fetch(`http://127.0.0.1:${hostPort}`, {
        signal: AbortSignal.timeout(500),
    });
    expect(response.status).toBe(200);
    expect(await response.text()).toBe("Hello from FTL\n");
}, 5000);
