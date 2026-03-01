import { describe, expect, it } from "bun:test";
import { boot } from "./helpers/vm";

describe('Hello World', () => {
    it('prints a boot message', async () => {
        using vm = await boot({});
        await vm.waitForLogs(async (logs) => {
            expect(logs).toContainEqual(expect.stringContaining('Booting FTL...'));
        });
    });
});
