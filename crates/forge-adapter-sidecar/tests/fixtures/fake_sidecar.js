#!/usr/bin/env node
import * as readline from "node:readline";

const rl = readline.createInterface({ input: process.stdin, terminal: false });

function emit(event) {
  process.stdout.write(JSON.stringify(event) + "\n");
}

rl.on("line", (line) => {
  const trimmed = line.trim();
  if (trimmed.length === 0) return;
  const command = JSON.parse(trimmed);

  switch (command.type) {
    case "start_session":
      emit({ type: "session_started", session_id: "fake-session-1" });
      return;
    case "send_message":
      if (command.text === "TRIGGER_MALFORMED") {
        process.stdout.write("this line is not valid json\n");
        emit({ type: "assistant_delta", text: "recovered-after-malformed" });
        return;
      }
      if (command.text === "TRIGGER_LARGE") {
        emit({ type: "tool_result", tool_call_id: "t-1", output: "x".repeat(200000), is_error: false });
        return;
      }
      emit({ type: "assistant_delta", text: `echo:${command.text}` });
      return;
    case "permission_response":
      emit({ type: "assistant_delta", text: `permission:${command.decision}` });
      return;
    case "interrupt":
      emit({ type: "error", message: "interrupted" });
      return;
    case "shutdown":
      emit({ type: "session_ended" });
      process.exit(0);
  }
});
