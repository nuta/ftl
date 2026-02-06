export async function createInitfs(files: Record<string, string>): Promise<Bun.Archive> {
    const archive: Record<string, ArrayBuffer> = {};
    for (const [name, path] of Object.entries(files)) {
        archive[name] = await Bun.file(path).arrayBuffer();
    }

    return new Bun.Archive(archive);
}
