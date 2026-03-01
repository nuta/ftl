export interface WaitUntilOptions {
    timeoutMs?: number;
    intervalMs?: number;
}

export async function waitFor<T>(
    callback: () => Promise<T>,
    {
        timeoutMs = 5000,
        intervalMs = 100,
    }: WaitUntilOptions = {}
): Promise<T> {
    const deadline = Date.now() + timeoutMs;
    let lastError: unknown = null;
    while (Date.now() < deadline) {
        try {
            return await callback();
        } catch (error) {
            lastError = error;
        }

        await new Promise((resolve) => setTimeout(resolve, intervalMs));
    }

    throw new Error(`waitFor timed out`, { cause: lastError });
}
