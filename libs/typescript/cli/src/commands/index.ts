import * as build from "./build";
import * as run from "./run";
import * as dev from "./dev";

export interface Command {
    main: (args: string[]) => Promise<void>;
}

export const COMMANDS: Record<string, Command> = {
    build,
    run,
    dev,
}
