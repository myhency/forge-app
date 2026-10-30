import { existsSync, mkdtempSync, mkdirSync, readFileSync, readdirSync, readlinkSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { isSafeSkillName } from "../src/skills_plugin.js";
import { SidecarSession, DENIED_MESSAGE, QUESTION_SKIPPED_MESSAGE, type QueryFn } from "../src/session.js";
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
  prompt_suggestions: true,
  allow_bypass: true,
  dynamic_workflows: false,
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
  it("adds the ccd_turn server and allows its two tools when turn_tools is on", async () => {
    let captured: Record<string, any> | undefined;
    const queryFn: QueryFn = ({ options }) => {
      captured = options;
      return toQueryHandle((async function* () {})());
    };
    const session = new SidecarSession(queryFn, () => {});
    await session.handle({
      ...startCommand,
      turn_tools: true,
      mcp_servers: { files: { command: "npx", args: ["files-mcp"] } },
      allowed_tools: ["Read"],
    });
    expect(Object.keys(captured!.mcpServers).sort()).toEqual(["ccd_turn", "files"]);
    expect(captured!.allowedTools).toEqual(["Read", "mcp__ccd_turn__update_plan", "mcp__ccd_turn__wrap_up"]);
  });

  it("adds the forge_ui server and allows its tool when generative_ui is on", async () => {
    let captured: Record<string, any> | undefined;
    const queryFn: QueryFn = ({ options }) => {
      captured = options;
      return toQueryHandle((async function* () {})());
    };
    await new SidecarSession(queryFn, () => {}).handle({ ...startCommand, generative_ui: true, allowed_tools: ["Read"] });
    expect(Object.keys(captured!.mcpServers)).toEqual(["forge_ui"]);
    expect(captured!.allowedTools).toEqual(["Read", "mcp__forge_ui__ui_guide", "mcp__forge_ui__generate_sandboxed_ui"]);
  });

  it("adds nothing when turn_tools is off or missing (an older parent)", async () => {
    const seen: Array<Record<string, any>> = [];
    const queryFn: QueryFn = ({ options }) => {
      seen.push(options);
      return toQueryHandle((async function* () {})());
    };
    await new SidecarSession(queryFn, () => {}).handle({ ...startCommand, turn_tools: false });
    await new SidecarSession(queryFn, () => {}).handle({ ...startCommand });
    for (const options of seen) {
      expect(options.mcpServers).toEqual({});
      expect(options.allowedTools).toEqual([]);
    }
  });

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

  it("forwards a streamed thinking delta as a thinking_delta event", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle((async function* () {
        yield {
          type: "stream_event",
          event: { type: "content_block_delta", index: 0, delta: { type: "thinking_delta", thinking: "Let me see" } },
        };
        yield {
          type: "stream_event",
          event: { type: "content_block_delta", index: 0, delta: { type: "signature_delta", signature: "abc" } },
        };
      })());

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toEqual([{ type: "thinking_delta", text: "Let me see" }, { type: "session_ended" }]);
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
            server_tool_use: { web_search_requests: 3, web_fetch_requests: 1 },
          },
          // A running total that includes helper models: it must not be what is sent.
          modelUsage: { "claude-test": { inputTokens: 999 }, "claude-helper": { inputTokens: 901 } },
        };
      })());

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toContainEqual({
      type: "turn_complete",
      usage: { "claude-test": { input: 12, output: 34, cache_read: 5, cache_write_5m: 2, cache_write_1h: 4, web_search_requests: 3 } },
    });
  });

  const searchResult = (n: unknown, parent: string | null = null) => ({
    type: "user",
    parent_tool_use_id: parent,
    message: { role: "user", content: [{ type: "tool_result", tool_use_id: "toolu_s", content: "..." }] },
    tool_use_result: { query: "rust", results: [], durationSeconds: 1, searchCount: n },
  });

  it("counts the web searches a WebSearch tool result reports in the turn's usage", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle((async function* () {
        yield { type: "assistant", parent_tool_use_id: null, message: { model: "claude-test", content: [] } };
        yield searchResult(2);
        yield { type: "result", subtype: "success", usage: { input_tokens: 5, output_tokens: 6, server_tool_use: { web_search_requests: 0 } } };
      })());

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    const done = events.find((e) => e.type === "turn_complete") as Extract<SidecarEvent, { type: "turn_complete" }>;
    expect(done.usage["claude-test"].web_search_requests).toBe(2);
    expect(done.usage["claude-test"].input).toBe(5);
  });

  it("does not carry one turn's searches into the next", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle((async function* () {
        yield { type: "assistant", parent_tool_use_id: null, message: { model: "claude-test", content: [] } };
        yield searchResult(2);
        yield { type: "result", subtype: "success", usage: { input_tokens: 1 } };
        yield { type: "assistant", parent_tool_use_id: null, message: { model: "claude-test", content: [] } };
        yield { type: "result", subtype: "success", usage: { input_tokens: 1 } };
      })());

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    const done = events.filter((e) => e.type === "turn_complete") as Array<Extract<SidecarEvent, { type: "turn_complete" }>>;
    expect(done.map((e) => e.usage["claude-test"].web_search_requests)).toEqual([2, 0]);
  });

  it("ignores a sub-agent's search and results that are not a search count", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle((async function* () {
        yield { type: "assistant", parent_tool_use_id: null, message: { model: "claude-test", content: [] } };
        yield searchResult(3, "toolu_parent");
        yield searchResult("many");
        yield searchResult(-1);
        yield { type: "user", parent_tool_use_id: null, message: { role: "user", content: [] }, tool_use_result: { toolStats: { searchCount: 9 } } };
        yield { type: "result", subtype: "success", usage: { input_tokens: 1 } };
      })());

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    const done = events.find((e) => e.type === "turn_complete") as Extract<SidecarEvent, { type: "turn_complete" }>;
    expect(done.usage["claude-test"].web_search_requests).toBe(0);
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

  function detailQuery(extra: Record<string, unknown>): QueryFn {
    return () =>
      toQueryHandle(
        (async function* () {
          yield { type: "result", subtype: "success", usage: {} };
          await new Promise((resolve) => setTimeout(resolve, 30));
        })(),
        extra,
      );
  }

  const usageWith = (extra: Record<string, unknown>) => async () => ({ totalTokens: 100, maxTokens: 1000, ...extra });

  it("sends context_detail with its categories after context_usage", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      detailQuery({
        getContextUsage: usageWith({
          categories: [
            { name: "Messages", tokens: 20100, kind: "used" },
            { name: "Free space", tokens: 888900, kind: "free" },
          ],
          mcpTools: [
            { name: "Make", serverName: "Claude Docs", tokens: 152 },
            { name: "Read", serverName: "Claude Docs", tokens: 90 },
          ],
          agents: [{ agentType: "Explore", source: "built-in", tokens: 80 }],
        }),
      }),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    const types = events.map((e) => e.type);
    expect(types.indexOf("context_detail")).toBeGreaterThan(types.indexOf("context_usage"));
    expect(events).toContainEqual({
      type: "context_detail",
      categories: [
        { name: "Messages", tokens: 20100, kind: "used" },
        { name: "Free space", tokens: 888900, kind: "free" },
      ],
      mcp_tools: [
        { server: "Claude Docs", name: "Make", tokens: 152 },
        { server: "Claude Docs", name: "Read", tokens: 90 },
      ],
      agents: [{ name: "Explore", tokens: 80 }],
    });
  });

  it("drops categories with a bad kind or a non-finite token count", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      detailQuery({
        getContextUsage: usageWith({
          categories: [
            { name: "Messages", tokens: 5, kind: "used" },
            { name: "Odd", tokens: 5, kind: "weird" },
            { name: "Nan", tokens: Number.NaN, kind: "used" },
            { name: "Neg", tokens: -1, kind: "free" },
            { name: "Buf", tokens: 7, kind: "buffer" },
            { name: "Def", tokens: 9, kind: "deferred" },
          ],
        }),
      }),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    const detail = events.find((e) => e.type === "context_detail") as Extract<SidecarEvent, { type: "context_detail" }>;
    expect(detail.categories.map((c) => c.name)).toEqual(["Messages", "Buf", "Def"]);
    expect(detail.mcp_tools).toEqual([]);
    expect(detail.agents).toEqual([]);
  });

  it("sends no context_detail when the SDK sent no categories", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(detailQuery({ getContextUsage: usageWith({}) }), (e) => events.push(e));
    await session.handle(startCommand);

    const types = events.map((e) => e.type);
    expect(types).toContain("context_usage");
    expect(types).not.toContain("context_detail");
  });

  it("sends mcp_status after a turn", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      detailQuery({
        mcpServerStatus: async () => [
          { name: "figma", status: "connected", scope: "user", source: "user", config: { type: "http" }, tools: [{ name: "a" }, { name: "b" }, { name: "c" }] },
          { name: "broken", status: "failed", error: "boom" },
        ],
      }),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    const types = events.map((e) => e.type);
    expect(types.indexOf("mcp_status")).toBeGreaterThan(types.indexOf("turn_complete"));
    expect(events).toContainEqual({
      type: "mcp_status",
      servers: [
        { name: "figma", status: "connected", scope: "user", source: "user", type: "http", tool_count: 3, error: null },
        { name: "broken", status: "failed", scope: null, source: null, type: null, tool_count: 0, error: "boom" },
      ],
    });
  });

  it("sends no mcp_status when mcpServerStatus rejects or times out", async () => {
    for (const mcpServerStatus of [async () => { throw new Error("nope"); }, () => new Promise<never>(() => {})]) {
      const events: SidecarEvent[] = [];
      const session = new SidecarSession(
        detailQuery({ mcpServerStatus }),
        (e) => events.push(e),
        { detailTimeoutMs: 10, log: () => {} },
      );
      await session.handle(startCommand);

      const types = events.map((e) => e.type);
      expect(types).toContain("turn_complete");
      expect(types).not.toContain("mcp_status");
      expect(types).not.toContain("error");
    }
  });

  it("rounds fractional tokens in context_detail and still sends it", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      detailQuery({
        getContextUsage: usageWith({
          categories: [{ name: "Messages", tokens: 12.5, kind: "used" }],
          mcpTools: [{ name: "Make", serverName: "Docs", tokens: 3.4 }],
          agents: [{ agentType: "Explore", tokens: 7.6 }],
        }),
      }),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    expect(events).toContainEqual({
      type: "context_detail",
      categories: [{ name: "Messages", tokens: 13, kind: "used" }],
      mcp_tools: [{ server: "Docs", name: "Make", tokens: 3 }],
      agents: [{ name: "Explore", tokens: 8 }],
    });
  });

  it("drops MCP tool rows without a string server name and agent rows with bad tokens or names", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      detailQuery({
        getContextUsage: usageWith({
          categories: [{ name: 5, tokens: 1, kind: "used" }, { name: "Ok", tokens: 1, kind: "used" }],
          mcpTools: [
            { name: "A", tokens: 1 },
            { name: "B", serverName: 4, tokens: 1 },
            { name: "C", serverName: "s", tokens: Number.NaN },
            { name: "D", serverName: "s", tokens: 2 },
          ],
          agents: [
            { agentType: "Good", tokens: 1 },
            { agentType: 9, tokens: 1 },
            { agentType: "Bad", tokens: Number.POSITIVE_INFINITY },
            { agentType: "Neg", tokens: -2 },
          ],
        }),
      }),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    const detail = events.find((e) => e.type === "context_detail") as Extract<SidecarEvent, { type: "context_detail" }>;
    expect(detail.categories.map((c) => c.name)).toEqual(["Ok"]);
    expect(detail.mcp_tools).toEqual([{ server: "s", name: "D", tokens: 2 }]);
    expect(detail.agents).toEqual([{ name: "Good", tokens: 1 }]);
  });

  it("sends mcp_status with an empty server list", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(detailQuery({ mcpServerStatus: async () => [] }), (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toContainEqual({ type: "mcp_status", servers: [] });
  });

  it("logs to stderr when the detail calls time out or reject, and the ring keeps its short timeout", async () => {
    const events: SidecarEvent[] = [];
    const lines: string[] = [];
    const session = new SidecarSession(
      detailQuery({
        getContextUsage: () => new Promise<never>(() => {}),
        mcpServerStatus: async () => { throw new Error("boom"); },
      }),
      (e) => events.push(e),
      { contextUsageTimeoutMs: 5, detailTimeoutMs: 20, log: (l) => lines.push(l) },
    );
    await session.handle(startCommand);

    expect(lines.some((l) => l.includes("getContextUsage"))).toBe(true);
    expect(lines.some((l) => l.includes("mcpServerStatus failed: boom"))).toBe(true);
    expect(events.map((e) => e.type)).not.toContain("error");
  });

  it("sends the breakdown after the ring has given up, when the answer comes within the detail timeout", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      detailQuery({
        getContextUsage: () =>
          new Promise((resolve) =>
            setTimeout(() => resolve({ totalTokens: 1, maxTokens: 10, categories: [{ name: "Messages", tokens: 1, kind: "used" }] }), 25),
          ),
      }),
      (e) => events.push(e),
      { contextUsageTimeoutMs: 5, detailTimeoutMs: 500, log: () => {} },
    );
    await session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 60));

    const types = events.map((e) => e.type);
    expect(types).not.toContain("context_usage");
    expect(types).toContain("context_detail");
  });

  function commandsQuery(
    supportedCommands: () => Promise<unknown>,
    initCommands?: string[],
    waitMs = 30,
  ): QueryFn {
    return () =>
      toQueryHandle(
        (async function* () {
          await new Promise((resolve) => setTimeout(resolve, waitMs));
          if (initCommands) yield { type: "system", subtype: "init", session_id: "s-1", slash_commands: initCommands };
          await new Promise((resolve) => setTimeout(resolve, waitMs));
        })(),
        { supportedCommands },
      );
  }

  function commandEvents(events: SidecarEvent[]) {
    return events.filter((e) => e.type === "commands_available");
  }

  it("sends commands_available from supportedCommands() before any message", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      commandsQuery(async () => [
        { name: "compact", description: "Free up context", argumentHint: "[instructions]" },
        { name: "review", description: "" },
      ]),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    expect(commandEvents(events)).toEqual([
      {
        type: "commands_available",
        commands: [
          { name: "compact", description: "Free up context", argument_hint: "[instructions]" },
          { name: "review" },
        ],
      },
    ]);
  });

  it("drops a leading slash, empty and non-string names and duplicates", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      commandsQuery(async () => [{ name: "/compact" }, { name: "compact" }, { name: "" }, { name: 3 }, null, { name: "/" }]),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    expect(commandEvents(events)).toEqual([{ type: "commands_available", commands: [{ name: "compact" }] }]);
  });

  it("does not send the init list again when it has the same names", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      commandsQuery(async () => [{ name: "compact", description: "Free up context" }], ["compact"]),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    expect(commandEvents(events)).toHaveLength(1);
  });

  it("sends the init list when its names differ, and when supportedCommands() gave nothing", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      commandsQuery(async () => [{ name: "compact" }], ["compact", "review"]),
      (e) => events.push(e),
    );
    await session.handle(startCommand);
    expect(commandEvents(events)).toEqual([
      { type: "commands_available", commands: [{ name: "compact" }] },
      { type: "commands_available", commands: [{ name: "compact" }, { name: "review" }] },
    ]);

    const later: SidecarEvent[] = [];
    const second = new SidecarSession(
      commandsQuery(async () => { throw new Error("nope"); }, ["compact"]),
      (e) => later.push(e),
    );
    await second.handle(startCommand);
    expect(commandEvents(later)).toEqual([{ type: "commands_available", commands: [{ name: "compact" }] }]);
  });

  it("keeps the descriptions when init lists the same names in another order", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      commandsQuery(
        async () => [{ name: "compact", description: "Free up context" }, { name: "review", description: "Review code" }],
        ["review", "compact"],
      ),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    expect(commandEvents(events)).toHaveLength(1);
  });

  it("keeps the descriptions of the names it already knew when init adds a name", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      commandsQuery(async () => [{ name: "compact", description: "Free up context" }], ["review", "compact"]),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    expect(commandEvents(events).at(-1)).toEqual({
      type: "commands_available",
      commands: [{ name: "review" }, { name: "compact", description: "Free up context" }],
    });
  });

  it("sends nothing and no error when supportedCommands() throws, and the session carries on", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      commandsQuery(async () => { throw new Error("nope"); }),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    expect(commandEvents(events)).toEqual([]);
    expect(events.map((e) => e.type)).toEqual(["session_ended"]);
  });

  it("gives up on a supportedCommands() that never answers", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      commandsQuery(() => new Promise(() => {}), undefined, 60),
      (e) => events.push(e),
      { commandsTimeoutMs: 10 },
    );
    await session.handle(startCommand);

    expect(commandEvents(events)).toEqual([]);
    expect(events.map((e) => e.type)).toEqual(["session_ended"]);
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

  it("tells Forge which rules 'always allow' would add, as the strings allowedTools takes", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = (args) => {
      const canUseTool = (args.options as any).canUseTool;
      return toQueryHandle((async function* () {
        await canUseTool("Bash", { command: "npm test" }, {
          requestId: "req-r",
          suggestions: [
            { type: "addRules", behavior: "allow", destination: "session", rules: [{ toolName: "Bash", ruleContent: "npm test:*" }, { toolName: "Read" }] },
            { type: "addRules", behavior: "deny", destination: "session", rules: [{ toolName: "Bash", ruleContent: "rm:*" }] },
            { type: "setMode", mode: "acceptEdits", destination: "session" },
          ],
        });
      })());
    };
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    const started = session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 0));
    const request = events.find((e) => e.type === "permission_request") as { rules?: string[] };
    expect(request.rules).toEqual(["Bash(npm test:*)", "Read"]);
    await session.handle({ type: "permission_response", id: "2", permission_request_id: "req-r", decision: "allow_once" });
    await started;
  });

  it("sends no rules when the SDK gave no suggestions", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = (args) => {
      const canUseTool = (args.options as any).canUseTool;
      return toQueryHandle((async function* () {
        await canUseTool("Bash", { command: "ls" }, { requestId: "req-n" });
      })());
    };
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    const started = session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 0));
    const request = events.find((e) => e.type === "permission_request") as { rules?: string[] };
    expect(request.rules).toEqual([]);
    await session.handle({ type: "permission_response", id: "2", permission_request_id: "req-n", decision: "deny" });
    await started;
  });

  async function answerWith(extra: Record<string, unknown>, decision: "deny" | "allow_once"): Promise<any> {
    let result: any;
    const queryFn: QueryFn = (args) => {
      const canUseTool = (args.options as any).canUseTool;
      return toQueryHandle((async function* () {
        result = await canUseTool("ExitPlanMode", { plan: "p" }, { requestId: "req-x" });
      })());
    };
    const session = new SidecarSession(queryFn, () => undefined);
    const started = session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 0));
    await session.handle({ type: "permission_response", id: "2", permission_request_id: "req-x", decision, ...extra } as SidecarCommand);
    await started;
    return result;
  }

  it("deny with a reason sends the reason as the message", async () => {
    const result = await answerWith({ reason: "  smaller plan please  " }, "deny");
    expect(result).toEqual({ behavior: "deny", message: "smaller plan please" });
  });

  it("deny with a blank reason keeps the default message", async () => {
    const result = await answerWith({ reason: "   " }, "deny");
    expect(result).toEqual({ behavior: "deny", message: DENIED_MESSAGE });
  });

  it("allow ignores the reason", async () => {
    const result = await answerWith({ reason: "ignored" }, "allow_once");
    expect(result).toEqual({ behavior: "allow" });
  });

  it("starts the SDK with the allowed tools it was given", async () => {
    let options: any;
    const queryFn: QueryFn = (args) => {
      options = args.options;
      return toQueryHandle((async function* () {})());
    };
    const session = new SidecarSession(queryFn, () => undefined);
    await session.handle({ ...startCommand, allowed_tools: ["Read", "Bash(ls:*)"] } as SidecarCommand);
    expect(options.allowedTools).toEqual(["Read", "Bash(ls:*)"]);
    const plain = new SidecarSession(queryFn, () => undefined);
    await plain.handle(startCommand);
    expect(options.allowedTools).toEqual([]);
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

  it("denies a tool call with the real app's text so the model stops", async () => {
    let result: any;
    const queryFn: QueryFn = (args) => {
      const canUseTool = (args.options as any).canUseTool;
      return toQueryHandle((async function* () {
        result = await canUseTool("Write", { file_path: "/tmp/a.txt", content: "x" }, { requestId: "d-1" });
        yield { type: "stream_event", event: { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "." } } };
      })());
    };
    const session = new SidecarSession(queryFn, () => undefined);
    const started = session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 0));
    await session.handle({ type: "permission_response", id: "9", permission_request_id: "d-1", decision: "deny" });
    await started;

    expect(result).toEqual({ behavior: "deny", message: DENIED_MESSAGE });
    expect(DENIED_MESSAGE).toBe(
      "The user doesn't want to proceed with this tool use. The tool use was rejected (eg. if it was a file edit, the new_string was NOT written to the file). STOP what you are doing and wait for the user to tell you how to proceed.",
    );
  });

  it("tells the parent when the SDK withdraws a pending request, and denies it", async () => {
    const events: SidecarEvent[] = [];
    let result: any;
    const controller = new AbortController();
    const queryFn: QueryFn = (args) => {
      const canUseTool = (args.options as any).canUseTool;
      return toQueryHandle((async function* () {
        result = await canUseTool("Bash", { command: "ls" }, { requestId: "w-1", signal: controller.signal });
        yield { type: "stream_event", event: { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "." } } };
      })());
    };
    const session = new SidecarSession(queryFn, (event) => events.push(event));
    const started = session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 0));
    controller.abort();
    await started;

    expect(result).toEqual({ behavior: "deny", message: "Interrupted" });
    expect(events).toContainEqual({ type: "permission_cancelled", permission_request_id: "w-1" });
    // The answer for a request that is gone changes nothing.
    await expect(session.handle({ type: "permission_response", id: "9", permission_request_id: "w-1", decision: "allow_once" })).resolves.toBeUndefined();
  });

  it("denies a skipped question with its own message", async () => {
    let result: any;
    const queryFn: QueryFn = (args) => {
      const canUseTool = (args.options as any).canUseTool;
      return toQueryHandle((async function* () {
        result = await canUseTool("AskUserQuestion", { questions: [] }, { requestId: "q-0" });
        yield { type: "stream_event", event: { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "." } } };
      })());
    };
    const session = new SidecarSession(queryFn, () => undefined);
    const started = session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 0));
    await session.handle({ type: "permission_response", id: "9", permission_request_id: "q-0", decision: "deny" });
    await started;

    expect(result).toEqual({ behavior: "deny", message: QUESTION_SKIPPED_MESSAGE });
    expect(QUESTION_SKIPPED_MESSAGE).toBe("The user skipped the question.");
  });

  it("answers a pending AskUserQuestion with updatedInput.answers", async () => {
    let result: any;
    const input = { questions: [{ question: "Q?", header: "H", options: [], multiSelect: false }] };
    const queryFn: QueryFn = (args) => {
      const canUseTool = (args.options as any).canUseTool;
      return toQueryHandle((async function* () {
        result = await canUseTool("AskUserQuestion", input, { requestId: "q-1" });
        yield { type: "stream_event", event: { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "." } } };
      })());
    };
    const session = new SidecarSession(queryFn, () => undefined);
    const started = session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 0));
    await session.handle({ type: "answer_question", id: "9", permission_request_id: "q-1", answers: { "Q?": "파랑,초록" } });
    await started;

    expect(result).toEqual({ behavior: "allow", updatedInput: { ...input, answers: { "Q?": "파랑,초록" } } });
  });

  it("ignores answer_question for an unknown id and for a request that is not a question", async () => {
    let result: any;
    let settled = false;
    const queryFn: QueryFn = (args) => {
      const canUseTool = (args.options as any).canUseTool;
      return toQueryHandle((async function* () {
        const pending = canUseTool("Bash", { command: "ls" }, { requestId: "b-1" });
        pending.then((r: any) => { result = r; settled = true; });
        yield { type: "stream_event", event: { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "." } } };
        await pending;
      })());
    };
    const session = new SidecarSession(queryFn, () => undefined);
    const started = session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 0));

    await session.handle({ type: "answer_question", id: "1", permission_request_id: "nope", answers: {} });
    await session.handle({ type: "answer_question", id: "2", permission_request_id: "b-1", answers: { a: "b" } });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(settled).toBe(false);

    await session.handle({ type: "permission_response", id: "3", permission_request_id: "b-1", decision: "allow_once" });
    await started;
    expect(result).toEqual({ behavior: "allow" });
  });

  function systemMessages(...messages: unknown[]): ReturnType<QueryFn> {
    return toQueryHandle((async function* () {
      for (const message of messages) yield message;
    })());
  }

  it("forwards the SDK's background task set, leaving ambient tasks out", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      systemMessages({
        type: "system",
        subtype: "background_tasks_changed",
        tasks: [
          { task_id: "b1", task_type: "local_bash", description: "Sleep for 60 seconds" },
          { task_id: "a1", task_type: "agent", description: "ambient", ambient: true },
          { task_type: "local_bash", description: "no id" },
        ],
      });
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toContainEqual({
      type: "background_tasks",
      tasks: [{ task_id: "b1", description: "Sleep for 60 seconds", task_type: "local_bash" }],
    });
  });

  it("forwards an empty background task set too, so the last task can end", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () => systemMessages({ type: "system", subtype: "background_tasks_changed", tasks: [] });
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);
    expect(events).toContainEqual({ type: "background_tasks", tasks: [] });
  });

  it("forwards a finished task, ignoring a status it does not know", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      systemMessages(
        { type: "system", subtype: "task_notification", task_id: "b1", status: "completed", summary: "done", output_file: "/x" },
        { type: "system", subtype: "task_notification", task_id: "b2", status: "weird", summary: "?", output_file: "/y" },
      );
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    const finished = events.filter((e) => e.type === "task_finished");
    expect(finished).toEqual([{ type: "task_finished", task_id: "b1", status: "completed", summary: "done" }]);
  });

  it("forwards a running task's token count and drops progress without one", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      systemMessages(
        { type: "system", subtype: "task_progress", task_id: "w1", description: "d", usage: { total_tokens: 99100, tool_uses: 1, duration_ms: 5000 } },
        { type: "system", subtype: "task_progress", task_id: "w2", description: "d" },
        { type: "system", subtype: "task_progress", description: "no id", usage: { total_tokens: 1, tool_uses: 0, duration_ms: 0 } },
      );
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events.filter((e) => e.type === "task_progress")).toEqual([{ type: "task_progress", task_id: "w1", total_tokens: 99100 }]);
  });

  it("asks for a title from the first message of a new session only, and passes it on", async () => {
    const events: SidecarEvent[] = [];
    const asked: string[] = [];
    let release!: () => void;
    const queryFn: QueryFn = () =>
      toQueryHandle(
        (async function* () {
          await new Promise<void>((resolve) => (release = resolve));
        })(),
        { generateSessionTitle: async (text: string) => { asked.push(text); return "  짧은 제목 "; } },
      );
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    const started = session.handle(startCommand);
    await session.handle({ type: "send_message", id: "2", text: "first message" });
    await session.handle({ type: "send_message", id: "3", text: "second message" });
    await new Promise((resolve) => setTimeout(resolve, 0));
    release();
    await started;

    expect(asked).toEqual(["first message"]);
    expect(events.filter((e) => e.type === "session_title")).toEqual([{ type: "session_title", title: "짧은 제목" }]);
  });

  it("does not ask for a title when it resumes a session", async () => {
    const asked: string[] = [];
    const queryFn: QueryFn = () => toQueryHandle((async function* () {})(), { generateSessionTitle: async (text: string) => { asked.push(text); return "x"; } });
    const session = new SidecarSession(queryFn, () => {});
    const started = session.handle({ ...startCommand, resume_session_id: "abc" });
    await session.handle({ type: "send_message", id: "2", text: "hello" });
    await started;
    expect(asked).toEqual([]);
  });

  it("forwards the usage limit state the SDK reports", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle((async function* () {
        yield { type: "rate_limit_event", rate_limit_info: { status: "allowed_warning", rateLimitType: "seven_day", utilization: 0.754, resetsAt: 1793178000 } };
        yield { type: "rate_limit_event", rate_limit_info: { status: "allowed" } };
        yield { type: "rate_limit_event" };
      })());
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);
    expect(events.filter((e) => e.type === "rate_limit")).toEqual([
      { type: "rate_limit", status: "allowed_warning", kind: "seven_day", percent: 75, resets_at: 1793178000 },
      { type: "rate_limit", status: "allowed" },
    ]);
  });

  it("tells the parent when the SDK writes its interrupt line", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle((async function* () {
        yield { type: "user", message: { role: "user", content: [{ type: "text", text: "[Request interrupted by user for tool use]" }] } };
        yield { type: "user", message: { role: "user", content: [{ type: "text", text: "an ordinary echo" }] } };
      })());
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);
    expect(events.filter((e) => e.type === "interrupted")).toEqual([{ type: "interrupted" }]);
  });

  it("forwards a compaction with the token counts the SDK reports", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      systemMessages(
        { type: "system", subtype: "compact_boundary", compact_metadata: { trigger: "manual", pre_tokens: 21702, post_tokens: 4290 } },
        { type: "system", subtype: "compact_boundary" },
      );
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events.filter((e) => e.type === "compacted")).toEqual([
      { type: "compacted", pre_tokens: 21702, post_tokens: 4290 },
      { type: "compacted" },
    ]);
  });

  it("forwards a finished task's tokens and time when the SDK reports them", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      systemMessages(
        {
          type: "system",
          subtype: "task_notification",
          task_id: "w1",
          status: "failed",
          summary: "boom",
          output_file: "/x",
          usage: { total_tokens: 157469, tool_uses: 3, duration_ms: 70914 },
        },
      );
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events.filter((e) => e.type === "task_finished")).toEqual([
      { type: "task_finished", task_id: "w1", status: "failed", summary: "boom", tokens: 157469, duration_ms: 70914 },
    ]);
  });

  it("forwards a prompt suggestion and drops an empty one", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      systemMessages({ type: "prompt_suggestion", suggestion: "run the tests" }, { type: "prompt_suggestion", suggestion: "" });
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events.filter((e) => e.type === "prompt_suggestion")).toEqual([{ type: "prompt_suggestion", text: "run the tests" }]);
  });

  it("starts the query with prompt suggestions and the per-task stop declaration", async () => {
    let options: any;
    const queryFn: QueryFn = (args) => {
      options = args.options;
      return toQueryHandle((async function* () {})());
    };
    const session = new SidecarSession(queryFn, () => undefined);
    await session.handle(startCommand);
    expect(options.promptSuggestions).toBe(true);
    expect(options.perTaskStopAffordance).toBe(true);
  });

  it("stops a task through the SDK, and shows a refusal as setting_failed without ending the session", async () => {
    const events: SidecarEvent[] = [];
    const stopped: string[] = [];
    let release!: () => void;
    const queryFn: QueryFn = () =>
      toQueryHandle(
        (async function* () {
          await new Promise<void>((resolve) => (release = resolve));
        })(),
        {
          stopTask: async (taskId: string) => {
            if (taskId === "gone") throw new Error("no such task");
            stopped.push(taskId);
          },
        },
      );
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    const started = session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 0));

    await session.handle({ type: "stop_task", id: "5", task_id: "b1" });
    await session.handle({ type: "stop_task", id: "6", task_id: "gone" });
    release();
    await started;

    expect(stopped).toEqual(["b1"]);
    expect(events).toContainEqual({ type: "setting_failed", message: "no such task" });
    expect(events.some((e) => e.type === "error")).toBe(false);
  });

  it("answers a side question without waiting for it, and says why when it cannot", async () => {
    const events: SidecarEvent[] = [];
    let release!: () => void;
    const asked: string[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle(
        (async function* () {
          await new Promise<void>((resolve) => (release = resolve));
        })(),
        {
          askSideQuestion: async (question: string) => {
            asked.push(question);
            if (question === "boom") throw new Error("model said no");
            if (question === "empty") return null;
            return { response: "41" };
          },
        },
      );
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    const started = session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 0));

    await session.handle({ type: "side_question", id: "1", request_id: "s1", question: "number?" });
    await session.handle({ type: "side_question", id: "2", request_id: "s2", question: "boom" });
    await session.handle({ type: "side_question", id: "3", request_id: "s3", question: "empty" });
    await new Promise((resolve) => setTimeout(resolve, 0));
    release();
    await started;

    expect(asked).toEqual(["number?", "boom", "empty"]);
    expect(events.filter((e) => e.type === "side_answer")).toEqual([
      { type: "side_answer", request_id: "s1", answer: "41" },
      { type: "side_answer", request_id: "s2", error: "model said no" },
      { type: "side_answer", request_id: "s3" },
    ]);
  });

  it("reports a side question as failed when the SDK has no such call", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(() => toQueryHandle((async function* () {})()), (e) => events.push(e), { queryWaitMs: 20 });
    await session.handle({ type: "side_question", id: "1", request_id: "s1", question: "q" });
    await new Promise((resolve) => setTimeout(resolve, 60));
    expect(events).toContainEqual({ type: "side_answer", request_id: "s1", error: "this version of the SDK cannot answer side questions" });
  });

  it("answers stop_task with setting_failed when no session is running", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(() => toQueryHandle((async function* () {})()), (e) => events.push(e));
    await session.handle({ type: "stop_task", id: "1", task_id: "b1" });
    expect(events).toContainEqual({ type: "setting_failed", message: "no session is running" });
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

  it("builds image content for a send_message with attachments and passes additional directories", async () => {
    let captured: any;
    const received: unknown[] = [];
    const queryFn: QueryFn = (args) => {
      captured = args.options;
      return toQueryHandle((async function* () {
        for await (const msg of args.prompt as AsyncIterable<{ message: { content: unknown } }>) {
          received.push(msg.message.content);
          return;
        }
      })());
    };

    const session = new SidecarSession(queryFn, () => {});
    const startPromise = session.handle({ ...startCommand, additional_directories: ["/extra"] });
    await session.handle({ type: "send_message", id: "2", text: "hi", attachments: [{ kind: "file", path: "/x/b.txt" }] });
    await startPromise;

    expect(captured.additionalDirectories).toEqual(["/extra"]);
    expect(received).toEqual(["hi\n@/x/b.txt"]);
  });

  it("gives every message it hands to the SDK a uuid and announces it", async () => {
    const events: SidecarEvent[] = [];
    const received: Array<{ uuid?: string }> = [];
    const queryFn: QueryFn = (args) =>
      toQueryHandle((async function* () {
        for await (const msg of args.prompt as AsyncIterable<{ uuid?: string }>) {
          received.push(msg);
          if (received.length === 2) return;
        }
      })());
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    const started = session.handle(startCommand);
    await session.handle({ type: "send_message", id: "2", text: "first" });
    await session.handle({ type: "send_message", id: "3", text: "second" });
    await started;

    const announced = events.filter((e) => e.type === "user_message").map((e) => (e as { uuid: string }).uuid);
    expect(announced).toHaveLength(2);
    expect(announced[0]).not.toEqual(announced[1]);
    expect(received.map((m) => m.uuid)).toEqual(announced);
  });

  it("turns file checkpointing on so the files can be rewound", async () => {
    let captured: any;
    const queryFn: QueryFn = (args) => {
      captured = args.options;
      return toQueryHandle((async function* () {})());
    };
    await new SidecarSession(queryFn, () => {}).handle(startCommand);
    expect(captured.enableFileCheckpointing).toBe(true);
  });

  it("rewinds the files of a message, or previews it, and says why it cannot", async () => {
    const events: SidecarEvent[] = [];
    const calls: Array<[string, unknown]> = [];
    let release!: () => void;
    const queryFn: QueryFn = () =>
      toQueryHandle(
        (async function* () {
          await new Promise<void>((resolve) => (release = resolve));
        })(),
        {
          rewindFiles: async (id: string, options?: { dryRun?: boolean }) => {
            calls.push([id, options?.dryRun]);
            if (id === "bad") throw new Error("no checkpoint");
            if (id === "none") return { canRewind: false, error: "nothing to rewind" };
            return { canRewind: true, filesChanged: ["/a/b.txt"], insertions: 1, deletions: 2 };
          },
        },
      );
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    const started = session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 0));

    await session.handle({ type: "rewind_files", id: "1", request_id: "r1", message_uuid: "m1", dry_run: true });
    await session.handle({ type: "rewind_files", id: "2", request_id: "r2", message_uuid: "none", dry_run: false });
    await session.handle({ type: "rewind_files", id: "3", request_id: "r3", message_uuid: "bad", dry_run: false });
    await new Promise((resolve) => setTimeout(resolve, 20));
    release();
    await started;

    expect(calls).toEqual([["m1", true], ["none", false], ["bad", false]]);
    expect(events.filter((e) => e.type === "rewind_result")).toEqual([
      { type: "rewind_result", request_id: "r1", can_rewind: true, files_changed: ["/a/b.txt"], insertions: 1, deletions: 2 },
      { type: "rewind_result", request_id: "r2", can_rewind: false, error: "nothing to rewind" },
      { type: "rewind_result", request_id: "r3", can_rewind: false, error: "no checkpoint" },
    ]);
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

  it("passes the command's env over the process environment, and none when there is none", async () => {
    const seen: any[] = [];
    const queryFn: QueryFn = (args) => {
      seen.push(args.options);
      return toQueryHandle((async function* () {})());
    };
    process.env.FORGE_TEST_KEEP = "kept";
    try {
      const withEnv = new SidecarSession(queryFn, () => {});
      await withEnv.handle({ ...startCommand, env: { FORGE_TEST_VAR: "v", FORGE_TEST_KEEP: "replaced" } });
      const without = new SidecarSession(queryFn, () => {});
      await without.handle({ ...startCommand, id: "2" });
    } finally {
      delete process.env.FORGE_TEST_KEEP;
    }
    expect(seen[0].env.FORGE_TEST_VAR).toBe("v");
    expect(seen[0].env.FORGE_TEST_KEEP).toBe("replaced");
    expect(seen[0].env.PATH).toBe(process.env.PATH);
    expect(seen[1].env).toBeUndefined();
  });

  it("asks the SDK for a summary of the thinking, which it omits by default", async () => {
    let capturedOptions: any;
    const queryFn: QueryFn = (args) => {
      capturedOptions = args.options;
      return toQueryHandle((async function* () {})());
    };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle(startCommand);

    expect(capturedOptions.thinking).toEqual({ type: "adaptive", display: "summarized" });
  });

  describe("changing the mode and the model of a running session", () => {
    // A query that stays open until `stop()`, and records what the session asked of it.
    function openQuery(extra: Record<string, unknown> = {}) {
      let stopped = false;
      const calls: { mode?: unknown[]; model?: unknown[] } = { mode: [], model: [] };
      const queryFn: QueryFn = () => {
        const gen = (async function* () {
          while (!stopped) {
            await new Promise((resolve) => setTimeout(resolve, 1));
            yield { type: "noop" };
          }
        })();
        return Object.assign(
          gen,
          {
            interrupt: async () => undefined,
            setPermissionMode: async (mode: unknown) => void calls.mode!.push(mode),
            setModel: async (model: unknown) => void calls.model!.push(model),
          },
          extra,
        );
      };
      return { queryFn, calls, stop: () => (stopped = true) };
    }

    async function running(open: ReturnType<typeof openQuery>) {
      const events: SidecarEvent[] = [];
      const session = new SidecarSession(open.queryFn, (e) => events.push(e));
      const started = session.handle(startCommand);
      await new Promise((resolve) => setTimeout(resolve, 5));
      return { session, events, finish: async () => (open.stop(), await started) };
    }

    it("sets the SDK permission mode and says so", async () => {
      const open = openQuery();
      const { session, events, finish } = await running(open);

      await session.handle({ type: "set_permission_mode", id: "2", mode: "accept_edits" });
      await session.handle({ type: "set_permission_mode", id: "3", mode: "auto" });
      await finish();

      expect(open.calls.mode).toEqual(["acceptEdits", "auto"]);
      expect(events).toContainEqual({ type: "permission_mode_set", mode: "accept_edits" });
      expect(events).toContainEqual({ type: "permission_mode_set", mode: "auto" });
    });

    it("sets the model, and null means the SDK default", async () => {
      const open = openQuery();
      const { session, events, finish } = await running(open);

      await session.handle({ type: "set_model", id: "2", model: "claude-opus-5-5" });
      await session.handle({ type: "set_model", id: "3", model: null });
      await finish();

      expect(open.calls.model).toEqual(["claude-opus-5-5", undefined]);
      expect(events).toContainEqual({ type: "model_set", model: "claude-opus-5-5" });
      expect(events).toContainEqual({ type: "model_set", model: null });
    });

    it("reports a refusal as setting_failed and not as a change", async () => {
      const open = openQuery({
        setPermissionMode: async () => {
          throw new Error("auto mode is not available for this model");
        },
        setModel: async () => {
          throw new Error("unknown model");
        },
      });
      const { session, events, finish } = await running(open);

      await session.handle({ type: "set_permission_mode", id: "2", mode: "auto" });
      await session.handle({ type: "set_model", id: "3", model: "nope" });
      await finish();

      expect(events).toContainEqual({ type: "setting_failed", message: "auto mode is not available for this model" });
      expect(events).toContainEqual({ type: "setting_failed", message: "unknown model" });
      expect(events.some((e) => e.type === "permission_mode_set" || e.type === "model_set")).toBe(false);
      expect(events.some((e) => e.type === "error")).toBe(false);
    });

    it("says setting_failed when no session is running", async () => {
      const events: SidecarEvent[] = [];
      const session = new SidecarSession(() => toQueryHandle((async function* () {})()), (e) => events.push(e));

      await session.handle({ type: "set_permission_mode", id: "1", mode: "plan" });
      await session.handle({ type: "set_model", id: "2", model: "m" });

      expect(events.map((e) => e.type)).toEqual(["setting_failed", "setting_failed"]);
    });

    it("starts the SDK in auto mode when asked to", async () => {
      let captured: any;
      const queryFn: QueryFn = (args) => {
        captured = args.options;
        return toQueryHandle((async function* () {})());
      };
      await new SidecarSession(queryFn, () => {}).handle({ ...startCommand, permission_mode: "auto" });

      expect(captured.permissionMode).toBe("auto");
    });

    it("allows bypassPermissions to be chosen later, which the SDK requires", async () => {
      let captured: any;
      const queryFn: QueryFn = (args) => {
        captured = args.options;
        return toQueryHandle((async function* () {})());
      };
      await new SidecarSession(queryFn, () => {}).handle(startCommand);

      expect(captured.allowDangerouslySkipPermissions).toBe(true);
    });
  });

  describe("the Code tab defaults", () => {
    function capture(): { options: () => any; queryFn: QueryFn } {
      let captured: any;
      const queryFn: QueryFn = (args) => {
        captured = args.options;
        return toQueryHandle((async function* () {})());
      };
      return { options: () => captured, queryFn };
    }

    it("passes the output style as an inline setting, and no setting when there is none", async () => {
      const styled = capture();
      await new SidecarSession(styled.queryFn, () => {}).handle({ ...startCommand, output_style: "Concise" });
      expect(styled.options().settings).toEqual({ outputStyle: "Concise" });

      const plain = capture();
      await new SidecarSession(plain.queryFn, () => {}).handle(startCommand);
      expect(plain.options().settings).toBeUndefined();
    });

    it("turns the Workflow tool on only when the command asks, next to the style", async () => {
      const on = capture();
      await new SidecarSession(on.queryFn, () => {}).handle({ ...startCommand, dynamic_workflows: true });
      expect(on.options().settings).toEqual({ enableWorkflows: true });

      const both = capture();
      await new SidecarSession(both.queryFn, () => {}).handle({ ...startCommand, output_style: "Concise", dynamic_workflows: true });
      expect(both.options().settings).toEqual({ outputStyle: "Concise", enableWorkflows: true });
    });

    it("follows the command for prompt suggestions", async () => {
      const off = capture();
      await new SidecarSession(off.queryFn, () => {}).handle({ ...startCommand, prompt_suggestions: false });
      expect(off.options().promptSuggestions).toBe(false);
    });

    it("lets the SDK enter bypass only when the command allows it", async () => {
      const refused = capture();
      await new SidecarSession(refused.queryFn, () => {}).handle({ ...startCommand, allow_bypass: false });
      expect(refused.options().allowDangerouslySkipPermissions).toBe(false);

      const allowed = capture();
      await new SidecarSession(allowed.queryFn, () => {}).handle({ ...startCommand, allow_bypass: true });
      expect(allowed.options().allowDangerouslySkipPermissions).toBe(true);
    });
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

  describe("effort and the models the SDK lists", () => {
    function openQuery(extra: Record<string, unknown> = {}) {
      let stopped = false;
      const flags: unknown[] = [];
      const queryFn: QueryFn = () => {
        const gen = (async function* () {
          while (!stopped) {
            await new Promise((resolve) => setTimeout(resolve, 1));
            yield { type: "noop" };
          }
        })();
        return Object.assign(
          gen,
          { interrupt: async () => undefined, applyFlagSettings: async (settings: unknown) => void flags.push(settings) },
          extra,
        );
      };
      return { queryFn, flags, stop: () => (stopped = true) };
    }

    async function running(open: ReturnType<typeof openQuery>) {
      const events: SidecarEvent[] = [];
      const session = new SidecarSession(open.queryFn, (e) => events.push(e), { commandsTimeoutMs: 50 });
      const started = session.handle(startCommand);
      await new Promise((resolve) => setTimeout(resolve, 10));
      return { session, events, finish: async () => (open.stop(), await started) };
    }

    it("sets the effort through applyFlagSettings and says so; null clears it", async () => {
      const open = openQuery();
      const { session, events, finish } = await running(open);

      await session.handle({ type: "set_effort", id: "2", effort: "xhigh" });
      await session.handle({ type: "set_effort", id: "3", effort: null });
      await finish();

      expect(open.flags).toEqual([{ effortLevel: "xhigh" }, { effortLevel: null }]);
      expect(events).toContainEqual({ type: "effort_set", effort: "xhigh" });
      expect(events).toContainEqual({ type: "effort_set", effort: null });
    });

    it("reports a refusal as setting_failed and not as a change", async () => {
      const open = openQuery({
        applyFlagSettings: async () => {
          throw new Error("effort is capped by your organization");
        },
      });
      const { session, events, finish } = await running(open);

      await session.handle({ type: "set_effort", id: "2", effort: "max" });
      await finish();

      expect(events).toContainEqual({ type: "setting_failed", message: "effort is capped by your organization" });
      expect(events.some((e) => e.type === "effort_set" || e.type === "error")).toBe(false);
    });

    it("says setting_failed when no session runs, and when the SDK cannot change effort", async () => {
      const idle: SidecarEvent[] = [];
      await new SidecarSession(() => toQueryHandle((async function* () {})()), (e) => idle.push(e)).handle({ type: "set_effort", id: "1", effort: "low" });
      expect(idle.map((e) => e.type)).toEqual(["setting_failed"]);

      const open = openQuery({ applyFlagSettings: undefined });
      const { session, events, finish } = await running(open);
      await session.handle({ type: "set_effort", id: "2", effort: "low" });
      await finish();
      expect(events.some((e) => e.type === "setting_failed")).toBe(true);
      expect(events.some((e) => e.type === "effort_set")).toBe(false);
    });

    it("starts the SDK with the effort it is told, and without one when none is", async () => {
      const seen: any[] = [];
      const queryFn: QueryFn = (args) => {
        seen.push(args.options);
        return toQueryHandle((async function* () {})());
      };
      await new SidecarSession(queryFn, () => {}).handle({ ...startCommand, effort: "high" });
      await new SidecarSession(queryFn, () => {}).handle(startCommand);

      expect(seen[0].effort).toBe("high");
      expect(seen[1].effort).toBeUndefined();
    });

    it("sends the models the SDK lists, with only the effort levels Forge knows", async () => {
      const open = openQuery({
        supportedModels: async () => [
          { value: "opus", resolvedModel: "claude-opus-5-5", displayName: "Opus 5.5", supportsEffort: true, supportedEffortLevels: ["low", "high", "ultra"] },
          { value: "claude-haiku-4-5-20251001", displayName: "Haiku 4.5", supportsEffort: false },
          { displayName: "no value" },
        ],
      });
      const { events, finish } = await running(open);
      await finish();

      expect(events).toContainEqual({
        type: "models_available",
        models: [
          { value: "opus", resolved_model: "claude-opus-5-5", display_name: "Opus 5.5", effort_levels: ["low", "high"] },
          { value: "claude-haiku-4-5-20251001", display_name: "Haiku 4.5", effort_levels: [] },
        ],
      });
    });

    it("sends nothing when the model list fails or is empty", async () => {
      for (const supportedModels of [
        async () => {
          throw new Error("no");
        },
        async () => [],
      ]) {
        const open = openQuery({ supportedModels });
        const { events, finish } = await running(open);
        await finish();
        expect(events.some((e) => e.type === "models_available")).toBe(false);
      }
    });
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
