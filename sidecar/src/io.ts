import * as readline from "node:readline";
import { parseCommand, serializeEvent, type SidecarCommand, type SidecarEvent } from "./protocol.js";

export function readCommands(
  input: NodeJS.ReadableStream,
  onCommand: (command: SidecarCommand) => void,
  onError: (error: Error) => void,
): void {
  const rl = readline.createInterface({ input, terminal: false });
  rl.on("line", (line) => {
    const trimmed = line.trim();
    if (trimmed.length === 0) return;
    try {
      onCommand(parseCommand(trimmed));
    } catch (err) {
      onError(err instanceof Error ? err : new Error(String(err)));
    }
  });
}

export function writeEvent(output: NodeJS.WritableStream, event: SidecarEvent): void {
  output.write(serializeEvent(event) + "\n");
}
