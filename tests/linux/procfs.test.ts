import { test } from "bun:test";

import { boot, getAvailablePort } from "../helpers/vm";

test("/proc/version", async () => {
    const hostPort = await getAvailablePort();
    using vm = await boot({ hostPort, init: "/bin/easybox cat /proc/version" });
    await vm.waitForLog("\r\nFTL version ");
});

test("/proc/mounts", async () => {
    const hostPort = await getAvailablePort();
    using vm = await boot({ hostPort, init: "/bin/easybox cat /proc/mounts" });
    await vm.waitForLog("proc /proc proc rw 0 0");
});
