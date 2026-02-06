import * as build from "./build";
import * as run from "./run";

export interface Command {
    run: (args: string[]) => Promise<void>;
}

export const COMMANDS: Record<string, Command> = {
    build,
    run,
}
