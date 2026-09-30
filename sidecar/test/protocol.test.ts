import { describe, expect, it } from "vitest";
import { parseCommand, serializeEvent } from "../src/protocol.js";

describe("parseCommand", () => {
  it("keeps the extension fields of a start_session command", () => {
    const line = JSON.stringify({
      type: "start_session",
      id: "1",
      cwd: "/tmp",
      resume_session_id: null,
      permission_mode: "default",
      model: null,
      skills: { pdf: "/skills/pdf" },
      plugins: ["/plugins/hud"],
      mcp_servers: { files: { command: "npx" } },
    });
    expect(parseCommand(line)).toMatchObject({
      skills: { pdf: "/skills/pdf" },
      plugins: ["/plugins/hud"],
      mcp_servers: { files: { command: "npx" } },
    });
  });

  it("parses a start_session command", () => {
    const line = JSON.stringify({
      type: "start_session",
      id: "1",
      cwd: "/tmp",
      resume_session_id: null,
      permission_mode: "default",
      model: null,
    });
    expect(parseCommand(line)).toEqual({
      type: "start_session",
      id: "1",
      cwd: "/tmp",
      resume_session_id: null,
      permission_mode: "default",
      model: null,
    });
  });

  it("throws on a line missing a type field", () => {
    expect(() => parseCommand("{}")).toThrow();
  });

  it("throws on invalid JSON", () => {
    expect(() => parseCommand("not json")).toThrow();
  });
});

describe("serializeEvent", () => {
  it("serializes a session_started event", () => {
    expect(JSON.parse(serializeEvent({ type: "session_started", session_id: "abc" }))).toEqual({
      type: "session_started",
      session_id: "abc",
    });
  });

  it("serializes a permission_request event", () => {
    const event = {
      type: "permission_request" as const,
      permission_request_id: "perm-1",
      tool_name: "Bash",
      input: { command: "ls" },
    };
    expect(JSON.parse(serializeEvent(event))).toEqual(event);
  });
});
