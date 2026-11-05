import { describe, expect, it } from "vitest";
import { createAppToolsServers, READ_ONLY_APP_TOOLS, sessionMgmtTools } from "../src/app_tools.js";
import { SidecarSession } from "../src/session.js";
import type { SidecarEvent } from "../src/protocol.js";

describe("app tools", () => {
  it("offer the desktop app's server and tool names", () => {
    expect(Object.keys(createAppToolsServers(async () => ({ output: "", is_error: false })))).toEqual(["ccd_session_mgmt", "ccd_sidebar", "ccd_view", "ccd_settings"]);
    expect(sessionMgmtTools(async () => ({ output: "", is_error: false })).map((t) => t.name)).toEqual(["list_sessions", "get_session", "send_message", "search_session_transcripts", "list_events", "get_usage", "set_session_title", "set_session_model", "set_session_effort", "set_session_permission_mode", "archive_session", "unarchive_session", "stop_session"]);
    expect(READ_ONLY_APP_TOOLS).not.toContain("mcp__ccd_session_mgmt__send_message");
  });

  it("forward a call to the app and pass its answer back", async () => {
    const asked: Array<[string, Record<string, unknown>]> = [];
    const [list] = sessionMgmtTools(async (tool, input) => {
      asked.push([tool, input]);
      return { output: "[]", is_error: false };
    });
    const result = await list.handler({ limit: 5 }, {});
    expect(asked).toEqual([["list_sessions", { limit: 5 }]]);
    expect(result.content[0].text).toBe("[]");
    expect(result.isError).toBeUndefined();
  });

  it("round-trip a request through the session: app_request out, app_response in", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession((() => { throw new Error("unused"); }) as never, (event) => events.push(event));
    const pending = session.askApp("get_settings", {});
    const request = events.find((event) => event.type === "app_request");
    expect(request).toMatchObject({ type: "app_request", tool: "get_settings", input: {} });
    await session.handle({ type: "app_response", id: "1", request_id: (request as { request_id: string }).request_id, output: "{}", is_error: false });
    await expect(pending).resolves.toEqual({ output: "{}", is_error: false });
  });
});
