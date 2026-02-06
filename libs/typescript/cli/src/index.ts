import { COMMANDS } from "./commands";

function help() {
    console.log('Usage: ftl <command> [options]');
    console.log('');
    console.log('Commands:');
    console.log('  build - Build the project');
    console.log('');
}

export async function main(args: string[]) {
    const commandName = args[0];
    const commandArgs = args.slice(1);

    if (!commandName) {
        help();
        process.exit(1);
    }

    const command = COMMANDS[commandName];
    if (!command) {
        console.error(`Unknown command: ${commandName}`);
        process.exit(1);
    }

    await command.run(commandArgs);
}
