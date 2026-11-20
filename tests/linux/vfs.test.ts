import { describe, test } from "bun:test";

import { boot, getAvailablePort } from "../helpers/vm";

describe("ls", () => {
    test("lists the root directory", async () => {
        const hostPort = await getAvailablePort();
        using vm = await boot({ hostPort, init: "/bin/util ls /" });
        await vm.waitForLog("\r\nproc\r\n");
    });

    test("resolves . and .. in a path", async () => {
        const hostPort = await getAvailablePort();
        using vm = await boot({ hostPort, init: "/bin/util ls /dev/./../proc" });
        await vm.waitForLog("\r\nversion\r\n");
    });

    test("prints the path of a file", async () => {
        const hostPort = await getAvailablePort();
        using vm = await boot({ hostPort, init: "/bin/util ls /dev/null" });
        await vm.waitForLog("\r\n/dev/null\r\n");
    });
});

describe("pwd", () => {
    test("prints the directory after chdir", async () => {
        const hostPort = await getAvailablePort();
        using vm = await boot({ hostPort, init: "/bin/util pwd /dev/./../proc" });
        await vm.waitForLog("\r\n/proc\r\n");
    });
});

describe("cat", () => {
    test("reads a file", async () => {
        const hostPort = await getAvailablePort();
        using vm = await boot({ hostPort, init: "/bin/util cat /proc/version" });
        await vm.waitForLog("\r\nFTL version ");
    });

    test("fails on a missing file", async () => {
        const hostPort = await getAvailablePort();
        using vm = await boot({ hostPort, init: "/bin/util cat /missing" });
        await vm.waitForLog("cat: /missing: No such file or directory");
    });
});
