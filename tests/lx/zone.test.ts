import { describe, test } from "bun:test";

import { boot, getAvailablePort } from "../helpers/vm";

describe("zone", () => {
    test("stdout", async () => {
        const hostPort = await getAvailablePort();
        using vm = await boot({ hostPort, init: "/bin/util zone /bin/util echo hello from zone" });
        await vm.waitForLog("[zone:stdout] hello from zone");
    }, 5000);
});
