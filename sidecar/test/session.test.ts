import { existsSync, mkdtempSync, mkdirSync, readFileSync, readdirSync, readlinkSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { isSafeSkillName } from "../src/skills_plugin.js";
import { SidecarSession, type QueryFn } from "../src/session.js";
import type { SidecarCommand, SidecarEvent } from "../src/protocol.js";

function toQueryHandle(gen: AsyncGenerator<unknown>, extra: Record<string, unknown> = {}): ReturnType<QueryFn> {
  return Object.assign(gen, { interrupt: async () => undefined }, extra);
}

const startCommand: SidecarCommand = {
  type: "start_session",
  id: "1",
  cwd: "/tmp",
  resume_session_id: null,
  permission_mode: "default",
  model: null,
  skills: {},
  plugins: [],
  mcp_servers: {},
};

const skillRoots: string[] = [];

afterEach(() => {
  for (const root of skillRoots.splice(0)) rmSync(root, { recursive: true, force: true });
});

function forgeSkillsDirs(): string[] {
  return readdirSync(tmpdir()).filter((entry) => entry.startsWith("forge-skills-"));
}

function makeSkillDir(name: string): string {
  const root = mkdtempSync(join(tmpdir(), "forge-test-skill-"));
  skillRoots.push(root);
  const dir = join(root, name);
  mkdirSync(dir);
  writeFileSync(join(dir, "SKILL.md"), `---\nname: ${name}\ndescription: test\n---\n`);
  return dir;
}

describe("SidecarSession", () => {
  it("starts the SDK with only the project settings source and exactly the enabled extensions", async () => {
    let captured: Record<string, unknown> | undefined;
    const queryFn: QueryFn = ({ options }) => {
      captured = options;
      return toQueryHandle((async function* () {})());
    };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle({
      ...startCommand,
      plugins: ["/plugins/hud"],
      mcp_servers: { files: { command: "npx", args: ["files-mcp"] } },
    });

    expect(captured).toMatchObject({
      settingSources: ["project"],
      strictMcpConfig: true,
      skills: [],
      plugins: [{ type: "local", path: "/plugins/hud" }],
      mcpServers: { files: { command: "npx", args: ["files-mcp"] } },
    });
  });

  it("passes empty lists instead of omitting them, so the SDK defaults do not come back", async () => {
    let captured: Record<string, unknown> | undefined;
    const queryFn: QueryFn = ({ options }) => {
      captured = options;
      return toQueryHandle((async function* () {})());
    };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle(startCommand);

    expect(captured?.skills).toEqual([]);
    expect(captured?.plugins).toEqual([]);
    expect(captured?.mcpServers).toEqual({});
  });

  it("treats a command that lacks the extension fields as nothing enabled", async () => {
    let captured: Record<string, unknown> | undefined;
    const queryFn: QueryFn = ({ options }) => {
      captured = options;
      return toQueryHandle((async function* () {})());
    };
    const oldCommand = { type: "start_session", id: "1", cwd: "/tmp", resume_session_id: null, permission_mode: "default", model: null };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle(oldCommand as unknown as SidecarCommand);

    expect(captured?.skills).toEqual([]);
    expect(captured?.plugins).toEqual([]);
    expect(captured?.mcpServers).toEqual({});
  });

  it("delivers enabled skills through a generated forge-skills plugin and removes it when the session ends", async () => {
    const pdfDir = makeSkillDir("pdf");
    let pluginPath = "";
    let seen: { manifest: string; link: string } | undefined;
    let captured: Record<string, unknown> | undefined;
    const queryFn: QueryFn = ({ options }) => {
      captured = options;
      const plugins = options.plugins as { type: string; path: string }[];
      pluginPath = plugins[plugins.length - 1].path;
      seen = {
        manifest: readFileSync(join(pluginPath, ".claude-plugin", "plugin.json"), "utf8"),
        link: readlinkSync(join(pluginPath, "skills", "pdf")),
      };
      return toQueryHandle((async function* () {})());
    };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle({ ...startCommand, skills: { pdf: pdfDir }, plugins: ["/plugins/hud"] });

    expect(captured?.skills).toEqual(["forge-skills:pdf"]);
    expect((captured?.plugins as { path: string }[])[0].path).toBe("/plugins/hud");
    expect(JSON.parse(seen!.manifest)).toEqual({ name: "forge-skills" });
    expect(seen!.link).toBe(pdfDir);
    expect(existsSync(pluginPath)).toBe(false);
    expect(existsSync(join(pdfDir, "SKILL.md"))).toBe(true);
  });

  it("removes the generated plugin and rethrows the original error when queryFn throws synchronously", async () => {
    const pdfDir = makeSkillDir("pdf");
    let pluginPath = "";
    const queryFn: QueryFn = ({ options }) => {
      const plugins = options.plugins as { path: string }[];
      pluginPath = plugins[plugins.length - 1].path;
      throw new Error("sdk refused");
    };

    const session = new SidecarSession(queryFn, () => {});
    await expect(session.handle({ ...startCommand, skills: { pdf: pdfDir } })).rejects.toThrow("sdk refused");

    expect(pluginPath).not.toBe("");
    expect(existsSync(pluginPath)).toBe(false);
  });

  it("does not start the SDK when shutdown arrives while the plugin is being built, and still cleans up", async () => {
    const pdfDir = makeSkillDir("pdf");
    const before = new Set(forgeSkillsDirs());
    let called = false;
    const queryFn: QueryFn = () => {
      called = true;
      return toQueryHandle((async function* () {})());
    };

    const session = new SidecarSession(queryFn, () => {});
    const starting = session.handle({ ...startCommand, skills: { pdf: pdfDir } });
    const stopping = session.handle({ type: "shutdown", id: "2" });
    await Promise.all([starting, stopping]);
    await session.waitUntilEnded();

    expect(called).toBe(false);
    expect(forgeSkillsDirs().filter((entry) => !before.has(entry))).toEqual([]);
  });

  it("skips a skill whose target path is not absolute", async () => {
    const okDir = makeSkillDir("ok");
    let captured: Record<string, unknown> | undefined;
    const queryFn: QueryFn = ({ options }) => {
      captured = options;
      return toQueryHandle((async function* () {})());
    };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle({ ...startCommand, skills: { rel: "skills/rel", ok: okDir } });

    expect(captured?.skills).toEqual(["forge-skills:ok"]);
  });

  it("keeps one of two skills whose names differ only in case", async () => {
    const upper = makeSkillDir("PDF");
    const lower = makeSkillDir("pdf");
    let captured: Record<string, unknown> | undefined;
    const queryFn: QueryFn = ({ options }) => {
      captured = options;
      return toQueryHandle((async function* () {})());
    };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle({ ...startCommand, skills: { PDF: upper, pdf: lower } });

    expect(captured?.skills).toEqual(["forge-skills:PDF"]);
  });

  it("skips a skill whose name is not a safe path component", async () => {
    const okDir = makeSkillDir("ok");
    let captured: Record<string, unknown> | undefined;
    const queryFn: QueryFn = ({ options }) => {
      captured = options;
      return toQueryHandle((async function* () {})());
    };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle({
      ...startCommand,
      skills: { "../evil": okDir, "a/b": okDir, "..": okDir, ok: okDir },
    });

    expect(captured?.skills).toEqual(["forge-skills:ok"]);
  });

  it("generates no plugin when every skill name is unsafe", async () => {
    const okDir = makeSkillDir("ok");
    let captured: Record<string, unknown> | undefined;
    const queryFn: QueryFn = ({ options }) => {
      captured = options;
      return toQueryHandle((async function* () {})());
    };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle({ ...startCommand, skills: { "../evil": okDir } });

    expect(captured?.skills).toEqual([]);
    expect(captured?.plugins).toEqual([]);
  });

  it("forwards a session init and a streamed text delta as sidecar events, then session_ended", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle((async function* () {
        yield { type: "system", subtype: "init", session_id: "s-1" };
        yield {
          type: "stream_event",
          event: { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "Hello" } },
        };
      })());

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
      toQueryHandle((async function* () {
        yield {
          type: "assistant",
          message: { content: [{ type: "tool_use", id: "t-1", name: "Bash", input: { command: "ls" } }] },
        };
      })());

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toContainEqual({ type: "tool_use", tool_call_id: "t-1", name: "Bash", input: { command: "ls" } });
  });

  it("forwards a tool_result content block from a synthesized user message", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle((async function* () {
        yield {
          type: "user",
          message: {
            content: [{ type: "tool_result", tool_use_id: "t-1", content: [{ type: "text", text: "done" }], is_error: false }],
          },
        };
      })());

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toContainEqual({ type: "tool_result", tool_call_id: "t-1", output: "done", is_error: false });
  });

  it("forwards a result message as turn_complete with the turn's usage under the main-loop model", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle((async function* () {
        yield { type: "assistant", parent_tool_use_id: null, message: { model: "claude-test", content: [] } };
        yield {
          type: "result",
          subtype: "success",
          usage: {
            input_tokens: 12,
            output_tokens: 34,
            cache_read_input_tokens: 5,
            cache_creation_input_tokens: 6,
            cache_creation: { ephemeral_5m_input_tokens: 2, ephemeral_1h_input_tokens: 4 },
          },
          // A running total that includes helper models: it must not be what is sent.
          modelUsage: { "claude-test": { inputTokens: 999 }, "claude-helper": { inputTokens: 901 } },
        };
      })());

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toContainEqual({
      type: "turn_complete",
      usage: { "claude-test": { input: 12, output: 34, cache_read: 5, cache_write_5m: 2, cache_write_1h: 4 } },
    });
  });

  it("does not take a sub-agent's model for the turn", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle((async function* () {
        yield { type: "assistant", parent_tool_use_id: null, message: { model: "claude-main", content: [] } };
        yield { type: "assistant", parent_tool_use_id: "toolu_1", message: { model: "claude-sub", content: [] } };
        yield { type: "result", subtype: "success", usage: { input_tokens: 1, output_tokens: 2 } };
      })());

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    const done = events.find((e) => e.type === "turn_complete") as Extract<SidecarEvent, { type: "turn_complete" }>;
    expect(Object.keys(done.usage)).toEqual(["claude-main"]);
  });

  it("sends an empty usage for a turn that had no main-loop assistant message", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle((async function* () {
        yield { type: "result", subtype: "success", usage: { input_tokens: 1, output_tokens: 2 } };
      })());

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toContainEqual({ type: "turn_complete", usage: {} });
  });

  it("does not carry one turn's model into the next", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle((async function* () {
        yield { type: "assistant", parent_tool_use_id: null, message: { model: "claude-test", content: [] } };
        yield { type: "result", subtype: "success", usage: { input_tokens: 1, output_tokens: 1 } };
        yield { type: "result", subtype: "success", usage: { input_tokens: 7, output_tokens: 7 } };
      })());

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    const done = events.filter((e) => e.type === "turn_complete") as Array<Extract<SidecarEvent, { type: "turn_complete" }>>;
    expect(done.map((e) => Object.keys(e.usage).length)).toEqual([1, 0]);
  });

  function contextQuery(getContextUsage: () => Promise<unknown>, waitMs = 20): QueryFn {
    return () =>
      toQueryHandle(
        (async function* () {
          yield { type: "result", subtype: "success", usage: {} };
          await new Promise((resolve) => setTimeout(resolve, waitMs));
        })(),
        { getContextUsage },
      );
  }

  it("sends context_usage after turn_complete", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      contextQuery(async () => ({ totalTokens: 76200, maxTokens: 200000, percentage: 38 })),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    const types = events.map((e) => e.type);
    expect(types.indexOf("context_usage")).toBeGreaterThan(types.indexOf("turn_complete"));
    expect(events).toContainEqual({ type: "context_usage", total_tokens: 76200, max_tokens: 200000 });
  });

  it("sends no context_usage when getContextUsage throws, and the turn still completes", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      contextQuery(async () => { throw new Error("nope"); }),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    const types = events.map((e) => e.type);
    expect(types).toContain("turn_complete");
    expect(types).not.toContain("context_usage");
    expect(types).not.toContain("error");
  });

  it("gives up on a getContextUsage that never answers", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      contextQuery(() => new Promise(() => {}), 60),
      (e) => events.push(e),
      { contextUsageTimeoutMs: 10 },
    );
    await session.handle(startCommand);

    expect(events.map((e) => e.type)).not.toContain("context_usage");
  });

  it("ignores a context answer with an unusable window", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      contextQuery(async () => ({ totalTokens: 5, maxTokens: 0 })),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    expect(events.map((e) => e.type)).not.toContain("context_usage");
  });

  it("emits permission_request using the SDK's own requestId and resolves after permission_response (allow)", async () => {
    const events: SidecarEvent[] = [];
    let capturedCanUseTool: any;
    const queryFn: QueryFn = (args) => {
      capturedCanUseTool = (args.options as any).canUseTool;
      return toQueryHandle((async function* () {
        const result = await capturedCanUseTool("Bash", { command: "ls" }, { requestId: "req-1" });
        yield {
          type: "stream_event",
          event: {
            type: "content_block_delta",
            index: 0,
            delta: { type: "text_delta", text: result.behavior === "allow" ? "approved" : "denied" },
          },
        };
      })());
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
      return toQueryHandle((async function* () {
        const result = await capturedCanUseTool("Bash", { command: "rm -rf /" }, { requestId: "req-2" });
        yield {
          type: "stream_event",
          event: {
            type: "content_block_delta",
            index: 0,
            delta: { type: "text_delta", text: result.behavior === "deny" ? result.message : "should not happen" },
          },
        };
      })());
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
    const queryFn: QueryFn = () => toQueryHandle((async function* () {})());
    const session = new SidecarSession(queryFn, (e) => events.push(e));

    await expect(
      session.handle({ type: "permission_response", id: "1", permission_request_id: "does-not-exist", decision: "deny" }),
    ).resolves.toBeUndefined();
  });

  it("translates the wire protocol's snake_case permission_mode into the SDK's camelCase PermissionMode", async () => {
    let capturedPermissionMode: unknown;
    const queryFn: QueryFn = (args) => {
      capturedPermissionMode = (args.options as any).permissionMode;
      return toQueryHandle((async function* () {})());
    };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle({ ...startCommand, permission_mode: "accept_edits" });

    expect(capturedPermissionMode).toBe("acceptEdits");
  });

  it("translates a default permission_mode to the SDK's default", async () => {
    let capturedPermissionMode: unknown;
    const queryFn: QueryFn = (args) => {
      capturedPermissionMode = (args.options as any).permissionMode;
      return toQueryHandle((async function* () {})());
    };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle({ ...startCommand, permission_mode: "default" });

    expect(capturedPermissionMode).toBe("default");
  });

  it("delivers queued send_message commands to the prompt generator in order", async () => {
    const received: string[] = [];
    const queryFn: QueryFn = (args) =>
      toQueryHandle((async function* () {
        for await (const msg of args.prompt as AsyncIterable<{ message: { content: string } }>) {
          received.push(msg.message.content);
          if (received.length === 2) return;
        }
      })());

    const session = new SidecarSession(queryFn, () => {});
    const startPromise = session.handle(startCommand);
    await session.handle({ type: "send_message", id: "2", text: "first" });
    await session.handle({ type: "send_message", id: "3", text: "second" });
    await startPromise;

    expect(received).toEqual(["first", "second"]);
  });

  it("passes includePartialMessages: true to the SDK options", async () => {
    let capturedOptions: any;
    const queryFn: QueryFn = (args) => {
      capturedOptions = args.options;
      return toQueryHandle((async function* () {})());
    };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle(startCommand);

    expect(capturedOptions.includePartialMessages).toBe(true);
  });

  it("interrupt calls the query's own interrupt() method instead of aborting the whole session", async () => {
    let interruptCalled = false;
    const queryFn: QueryFn = () => {
      const gen = (async function* () {
        // yield forever so the session stays "active" until interrupted
        while (true) {
          await new Promise((resolve) => setTimeout(resolve, 1));
          yield { type: "result", subtype: "success", usage: { input_tokens: 0, output_tokens: 0 } };
        }
      })();
      return Object.assign(gen, {
        interrupt: async () => {
          interruptCalled = true;
        },
      });
    };

    const session = new SidecarSession(queryFn, () => {});
    void session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 5));

    await session.handle({ type: "interrupt", id: "2" });

    expect(interruptCalled).toBe(true);
  });

  it("allow_always returns the SDK's suggested permission updates as updatedPermissions", async () => {
    const events: SidecarEvent[] = [];
    let capturedResult: any;
    const suggestions = [{ type: "addRules", rules: [{ toolName: "Bash" }] }];
    const queryFn: QueryFn = (args) => {
      const canUseTool = (args.options as any).canUseTool;
      return toQueryHandle(
        (async function* () {
          capturedResult = await canUseTool("Bash", { command: "ls" }, { requestId: "req-3", suggestions });
        })(),
      );
    };

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    const startPromise = session.handle(startCommand);

    await new Promise((resolve) => setTimeout(resolve, 0));
    await session.handle({ type: "permission_response", id: "4", permission_request_id: "req-3", decision: "allow_always" });
    await startPromise;

    expect(capturedResult).toEqual({ behavior: "allow", updatedPermissions: suggestions });
  });
});

describe("isSafeSkillName", () => {
  it.each([
    ["", false],
    [".", false],
    ["..", false],
    ["a/b", false],
    ["a\\b", false],
    ["a\0b", false],
    ["pdf", true],
    ["my-skill_2", true],
  ])("%j -> %s", (name, expected) => {
    expect(isSafeSkillName(name)).toBe(expected);
  });
});
