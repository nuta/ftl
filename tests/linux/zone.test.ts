import { describe, test } from "bun:test";

import { boot, getAvailablePort } from "../helpers/vm";

describe("zone", () => {
    test("stdout", async () => {
        const hostPort = await getAvailablePort();
        using vm = await boot({ hostPort, init: "/bin/util zone /bin/util echo hello from zone" });
        await vm.waitForLog("[zone:stdout] hello from zone");
    }, 5000);

    test("stderr", async () => {
        const hostPort = await getAvailablePort();
        using vm = await boot({ hostPort, init: "/bin/util zone /bin/util cat /nonexistent" });
        await vm.waitForLog("[zone:stderr] cat: /nonexistent:");
    }, 5000);

    test("exit status", async () => {
        const hostPort = await getAvailablePort();
        using vm = await boot({ hostPort, init: "/bin/util zone /bin/util exit 42" });
        await vm.waitForLog("[zone:exit] status 42");
    }, 5000);
});
