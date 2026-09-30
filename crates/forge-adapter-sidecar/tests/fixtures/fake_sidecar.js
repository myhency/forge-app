#!/usr/bin/env node
import * as readline from "node:readline";
import { writeFileSync } from "node:fs";

const rl = readline.createInterface({ input: process.stdin, terminal: false });

function emit(event) {
  process.stdout.write(JSON.stringify(event) + "\n");
}

let started = null;

rl.on("line", (line) => {
  const trimmed = line.trim();
  if (trimmed.length === 0) return;
  const command = JSON.parse(trimmed);

  switch (command.type) {
    case "start_session":
      started = command;
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
      if (command.text === "TRIGGER_EXTENSIONS") {
        emit({
          type: "assistant_delta",
          text: JSON.stringify({ skills: started.skills, plugins: started.plugins, mcp_servers: started.mcp_servers }),
        });
        return;
      }
      if (command.text.startsWith("TRIGGER_ENV:")) {
        const name = command.text.slice("TRIGGER_ENV:".length);
        emit({ type: "assistant_delta", text: `env:${process.env[name] ?? "<unset>"}` });
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
      if (process.env.FORGE_TEST_IGNORE_SHUTDOWN) return;
      if (process.env.FORGE_TEST_EXIT_MARKER) writeFileSync(process.env.FORGE_TEST_EXIT_MARKER, "done");
      emit({ type: "session_ended" });
      process.exit(0);
  }
});
