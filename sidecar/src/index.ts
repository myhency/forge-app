import { query } from "@anthropic-ai/claude-agent-sdk";
import { readCommands, writeEvent } from "./io.js";
import { SidecarSession, type QueryFn } from "./session.js";
import type { SidecarEvent } from "./protocol.js";

function emit(event: SidecarEvent): void {
  writeEvent(process.stdout, event);
}

// `query`'s real signature is more specific than `QueryFn`; the cast documents
// the one boundary where this codebase's simplified type meets the SDK's own.
const session = new SidecarSession(query as unknown as QueryFn, emit);

readCommands(
  process.stdin,
  (command) => {
    void session
      .handle(command)
      .catch((err) => {
        emit({ type: "error", message: err instanceof Error ? err.message : String(err) });
      })
      .then(async () => {
        if (command.type === "shutdown") {
          await session.waitUntilEnded();
          process.exit(0);
        }
      });
  },
  (err) => {
    emit({ type: "error", message: `failed to parse command: ${err.message}` });
  },
);
