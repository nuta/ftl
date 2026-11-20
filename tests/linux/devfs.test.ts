import { test } from "bun:test";

import { boot, getAvailablePort } from "../helpers/vm";

test("/dev/null", async () => {
    const hostPort = await getAvailablePort();
    using vm = await boot({ hostPort, init: "/bin/util cat /dev/null" });
    // TODO: better assertion
    await vm.waitForLog("init process exited with status 0");
});
