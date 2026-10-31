import * as readline from "node:readline";
import { parseCommand, serializeEvent, type SidecarCommand, type SidecarEvent } from "./protocol.js";

export function readCommands(
  input: NodeJS.ReadableStream,
  onCommand: (command: SidecarCommand) => void,
  onError: (error: Error) => void,
  onClose: () => void = () => {},
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
  // The parent closes our stdin when it quits or dies. Without this the sidecar (and the `claude`
  // process its SDK query keeps open) would live on as an orphan.
  rl.on("close", onClose);
}

export function writeEvent(output: NodeJS.WritableStream, event: SidecarEvent): void {
  output.write(serializeEvent(event) + "\n");
}
