import { describe, expect, it } from "vitest";
import { createDesktopServer, desktopToolDefinitions, DESKTOP_SERVER_NAME, DESKTOP_TOOL, findBinary, INSTALL_HINT, toolsBinary } from "../src/agent_desktop.js";
import type { Runner } from "../src/agent_desktop.js";

async function call(runner: Runner, find: () => string | undefined, args: string[]) {
  const [definition] = desktopToolDefinitions({ runner, find });
  const done = (await definition.handler({ args }, {})) as { content: Array<{ text?: string }>; isError?: boolean };
  return { text: done.content.map((part) => part.text ?? "").join(""), isError: done.isError === true };
}

const ok: Runner = async () => ({ stdout: '{"ok":true}\n', stderr: "", code: 0, timedOut: false });

describe("agent_desktop tool", () => {
  it("names one tool on its own server and keeps it out of tool search", () => {
    expect(DESKTOP_SERVER_NAME).toBe("forge_desktop");
    expect(DESKTOP_TOOL).toBe("mcp__forge_desktop__agent_desktop");
    const list = desktopToolDefinitions();
    expect(list.map((tool) => tool.name)).toEqual(["agent_desktop"]);
    expect((list[0] as any)._meta?.["anthropic/alwaysLoad"]).toBe(true);
    expect(createDesktopServer().type).toBe("sdk");
  });

  it("runs the binary with the arguments as given, without a shell", async () => {
    let seen: { binary: string; args: string[] } | undefined;
    const runner: Runner = async (binary, args) => {
      seen = { binary, args };
      return ok(binary, args);
    };
    const result = await call(runner, () => "/bin/agent-desktop", ["click", "@s1:e2; rm -rf /"]);
    expect(seen).toEqual({ binary: "/bin/agent-desktop", args: ["click", "@s1:e2; rm -rf /"] });
    expect(result).toEqual({ text: '{"ok":true}', isError: false });
  });

  it("says how to install it when the binary is missing, and does not run anything", async () => {
    let ran = false;
    const result = await call(async () => ((ran = true), ok("", [])), () => undefined, ["snapshot"]);
    expect(ran).toBe(false);
    expect(result).toEqual({ text: INSTALL_HINT, isError: true });
  });

  it("hands back the JSON error of a failed command and marks it as an error", async () => {
    const failed: Runner = async () => ({ stdout: '{"ok":false,"error":{"code":"STALE_REF"}}', stderr: "", code: 1, timedOut: false });
    const result = await call(failed, () => "/bin/ad", ["click", "@s1:e2"]);
    expect(result.isError).toBe(true);
    expect(result.text).toContain("STALE_REF");
  });

  it("reports a timeout and cuts a very long answer", async () => {
    const slow: Runner = async () => ({ stdout: "", stderr: "", code: 1, timedOut: true });
    expect((await call(slow, () => "/bin/ad", ["snapshot"])).text).toContain("did not finish");
    const long: Runner = async () => ({ stdout: "x".repeat(150_000), stderr: "", code: 0, timedOut: false });
    const result = await call(long, () => "/bin/ad", ["snapshot"]);
    expect(result.text.length).toBeLessThan(101_000);
    expect(result.text).toContain("cut at 100000");
  });

  it("finds the binary on the usual install paths, or by AGENT_DESKTOP_BIN", () => {
    expect(findBinary({ PATH: "/usr/bin", HOME: "/h" }, (path) => path === "/opt/homebrew/bin/agent-desktop")).toBe("/opt/homebrew/bin/agent-desktop");
    expect(findBinary({ PATH: "", HOME: "/h", AGENT_DESKTOP_BIN: "/x/ad" }, (path) => path === "/x/ad")).toBe("/x/ad");
    expect(findBinary({ PATH: "/usr/bin", HOME: "/h" }, () => false)).toBeUndefined();
  });
});

describe("installed copy", () => {
  it("is found under the tools folder before the PATH, and only when it is there", () => {
    const placed = "/t/agent-desktop/node_modules/agent-desktop/bin/agent-desktop";
    const env = { PATH: "/usr/bin", HOME: "/h", FORGE_TOOLS_DIR: "/t" };
    expect(findBinary(env, (path) => path === placed || path === "/usr/bin/agent-desktop")).toBe(placed);
    expect(findBinary(env, (path) => path === "/usr/bin/agent-desktop")).toBe("/usr/bin/agent-desktop");
    expect(toolsBinary(undefined, () => true)).toBeUndefined();
  });
});
