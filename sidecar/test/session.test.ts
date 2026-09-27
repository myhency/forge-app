import { describe, expect, it } from "vitest";
import { SidecarSession, type QueryFn } from "../src/session.js";
import type { SidecarCommand, SidecarEvent } from "../src/protocol.js";

const startCommand: SidecarCommand = {
  type: "start_session",
  id: "1",
  cwd: "/tmp",
  resume_session_id: null,
  permission_mode: "default",
  model: null,
};

describe("SidecarSession", () => {
  it("forwards a session init and a streamed text delta as sidecar events, then session_ended", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      (async function* () {
        yield { type: "system", subtype: "init", session_id: "s-1" };
        yield {
          type: "stream_event",
          event: { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "Hello" } },
        };
      })();

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toEqual([
      { type: "session_started", session_id: "s-1" },
      { type: "assistant_delta", text: "Hello" },
      { type: "session_ended" },
    ]);
  });

  it("forwards a tool_use content block from an assistant message", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      (async function* () {
        yield {
          type: "assistant",
          message: { content: [{ type: "tool_use", id: "t-1", name: "Bash", input: { command: "ls" } }] },
        };
      })();

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toContainEqual({ type: "tool_use", tool_call_id: "t-1", name: "Bash", input: { command: "ls" } });
  });

  it("forwards a tool_result content block from a synthesized user message", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      (async function* () {
        yield {
          type: "user",
          message: {
            content: [{ type: "tool_result", tool_use_id: "t-1", content: [{ type: "text", text: "done" }], is_error: false }],
          },
        };
      })();

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toContainEqual({ type: "tool_result", tool_call_id: "t-1", output: "done", is_error: false });
  });

  it("forwards a result message as turn_complete", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      (async function* () {
        yield { type: "result", subtype: "success", usage: { input_tokens: 12, output_tokens: 34 } };
      })();

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toContainEqual({ type: "turn_complete", input_tokens: 12, output_tokens: 34 });
  });

  it("emits permission_request using the SDK's own requestId and resolves after permission_response (allow)", async () => {
    const events: SidecarEvent[] = [];
    let capturedCanUseTool: any;
    const queryFn: QueryFn = (args) => {
      capturedCanUseTool = (args.options as any).canUseTool;
      return (async function* () {
        const result = await capturedCanUseTool("Bash", { command: "ls" }, { requestId: "req-1" });
        yield {
          type: "stream_event",
          event: {
            type: "content_block_delta",
            index: 0,
            delta: { type: "text_delta", text: result.behavior === "allow" ? "approved" : "denied" },
          },
        };
      })();
    };

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    const startPromise = session.handle(startCommand);

    await new Promise((resolve) => setTimeout(resolve, 0));
    const permissionEvent = events.find((e) => e.type === "permission_request");
    expect(permissionEvent).toBeDefined();
    const requestId = (permissionEvent as { permission_request_id: string }).permission_request_id;
    expect(requestId).toBe("req-1");

    await session.handle({ type: "permission_response", id: "2", permission_request_id: requestId, decision: "allow_once" });
    await startPromise;

    expect(events.some((e) => e.type === "assistant_delta" && e.text === "approved")).toBe(true);
  });

  it("resolves with a PermissionResult deny (with a message) when the decision is deny", async () => {
    const events: SidecarEvent[] = [];
    let capturedCanUseTool: any;
    const queryFn: QueryFn = (args) => {
      capturedCanUseTool = (args.options as any).canUseTool;
      return (async function* () {
        const result = await capturedCanUseTool("Bash", { command: "rm -rf /" }, { requestId: "req-2" });
        yield {
          type: "stream_event",
          event: {
            type: "content_block_delta",
            index: 0,
            delta: { type: "text_delta", text: result.behavior === "deny" ? result.message : "should not happen" },
          },
        };
      })();
    };

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    const startPromise = session.handle(startCommand);

    await new Promise((resolve) => setTimeout(resolve, 0));
    await session.handle({ type: "permission_response", id: "3", permission_request_id: "req-2", decision: "deny" });
    await startPromise;

    const delta = events.find((e) => e.type === "assistant_delta") as { text: string } | undefined;
    expect(delta?.text).toBeTruthy();
    expect(delta?.text).not.toBe("should not happen");
  });

  it("ignores a permission_response for an unknown permission_request_id", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () => (async function* () {})();
    const session = new SidecarSession(queryFn, (e) => events.push(e));

    await expect(
      session.handle({ type: "permission_response", id: "1", permission_request_id: "does-not-exist", decision: "deny" }),
    ).resolves.toBeUndefined();
  });

  it("delivers queued send_message commands to the prompt generator in order", async () => {
    const received: string[] = [];
    const queryFn: QueryFn = (args) =>
      (async function* () {
        for await (const msg of args.prompt as AsyncIterable<{ message: { content: string } }>) {
          received.push(msg.message.content);
          if (received.length === 2) return;
        }
      })();

    const session = new SidecarSession(queryFn, () => {});
    const startPromise = session.handle(startCommand);
    await session.handle({ type: "send_message", id: "2", text: "first" });
    await session.handle({ type: "send_message", id: "3", text: "second" });
    await startPromise;

    expect(received).toEqual(["first", "second"]);
  });
});
