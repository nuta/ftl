import { test } from "bun:test";

import { boot, getAvailablePort } from "../helpers/vm";

test("/proc/version", async () => {
    const hostPort = await getAvailablePort();
    using vm = await boot({ hostPort, init: "/bin/util cat /proc/version" });
    await vm.waitForLog("\r\nFTL version ");
});
