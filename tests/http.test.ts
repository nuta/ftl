import { describe, expect, it } from "bun:test";
import { boot, getRandomPort, } from "./helpers/vm";
import { waitFor } from "./helpers/utils";

describe('HTTP server', () => {
    it('works', async () => {
        const hostPort = await getRandomPort();
        using vm = await boot({
            portForwarding: [
                {
                    protocol: "tcp",
                    hostPort,
                    guestPort: 80,
                },
            ],
        });

        await vm.waitForLogs(async (logs) => {
            expect(logs).toContainEqual(expect.stringContaining(`listening on 80`));
        });

        await waitFor(async () => {
            const resp = await fetch(`http://127.0.0.1:${hostPort}`, { signal: AbortSignal.timeout(200) });
            expect(resp.status).toBe(200);
            const body = await resp.text();
            expect(body).toContain('FTL operating system');
        });
    });
});
