import { mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { cleanName, draftStyle, parseDraft, saveStyle, styleFile, styleNameOk, userStylesDir } from "../src/output_styles.js";
import { SidecarSession, type QueryFn } from "../src/session.js";
import type { SidecarEvent } from "../src/protocol.js";

const dirs: string[] = [];
async function scratch(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "forge-styles-test-"));
  dirs.push(dir);
  return dir;
}
afterEach(async () => {
  for (const dir of dirs.splice(0)) await rm(dir, { recursive: true, force: true });
});

const draft = { suggestedName: "Pirate talk", description: "Talks like a pirate.", body: "# Pirate talk\n\nRespond like a pirate." };

describe("output styles", () => {
  it("live in the output-styles folder of the Claude home", () => {
    expect(userStylesDir({ CLAUDE_CONFIG_DIR: "/x/home" })).toBe(join("/x/home", "output-styles"));
    expect(userStylesDir({})).toMatch(/\.claude[\\/]output-styles$/);
  });

  it("read the drafter's JSON block and refuse anything else", () => {
    const text = 'Here:\n```json\n{"name":"Pirate talk","description":"Talks like a pirate.","body":"Respond like a pirate."}\n```';
    expect(parseDraft(text)).toEqual({ suggestedName: "Pirate talk", description: "Talks like a pirate.", body: "Respond like a pirate." });
    expect(parseDraft('{"description":"d","body":"b"}')).toEqual({ suggestedName: undefined, description: "d", body: "b" });
    expect(parseDraft("no json here")).toBeNull();
    expect(parseDraft('{"description":"d","body":"  "}')).toBeNull();
  });

  it("take only safe names", () => {
    expect(styleNameOk("간결한 리뷰-2")).toBe(true);
    expect(styleNameOk("../x")).toBe(false);
    expect(styleNameOk("   ")).toBe(false);
    expect(cleanName("Pirate/talk: v2!")).toBe("Pirate talk v2");
    expect(cleanName("///")).toBeUndefined();
  });

  it("write the desktop app's file: its own frontmatter, the drafter's dropped", () => {
    expect(styleFile("Pirate talk", "Talks\nlike a pirate.", "---\nname: x\n---\n\nArr.")).toBe(
      '---\nname: "Pirate talk"\ndescription: "Talks like a pirate."\n---\n\nArr.\n',
    );
  });

  it("save under a free name when Claude named it, and refuse a taken name the person typed", async () => {
    const dir = join(await scratch(), "output-styles");
    expect(await saveStyle(dir, "Pirate talk", draft, false)).toEqual({ ok: true, name: "Pirate talk" });
    expect(await saveStyle(dir, "Pirate talk", draft, false)).toEqual({ ok: true, name: "Pirate talk 2" });
    expect(await saveStyle(dir, "Pirate talk", draft, true)).toEqual({ ok: false, reason: "A style called Pirate talk already exists." });
    expect((await readdir(dir)).sort()).toEqual(["Pirate talk 2.md", "Pirate talk.md"]);
    expect(await readFile(join(dir, "Pirate talk.md"), "utf8")).toContain('name: "Pirate talk"');
  });

  it("never overwrite a file that is there", async () => {
    const dir = await scratch();
    await writeFile(join(dir, "Mine.md"), "keep me");
    expect(await saveStyle(dir, "Mine", draft, false)).toEqual({ ok: true, name: "Mine 2" });
    expect(await readFile(join(dir, "Mine.md"), "utf8")).toBe("keep me");
  });

  it("draft in one turn with no tools and the desktop app's prompt", async () => {
    let seen: Record<string, unknown> = {};
    const queryFn: QueryFn = ({ options }) => {
      seen = options;
      return (async function* () {
        yield { type: "assistant" };
        yield { type: "result", result: '```json\n{"name":"Pirate talk","description":"Arr.","body":"Respond like a pirate."}\n```' };
      })() as never;
    };
    const result = await draftStyle(queryFn, "like a pirate", undefined);
    expect(result).toEqual({ ok: true, draft: { suggestedName: "Pirate talk", description: "Arr.", body: "Respond like a pirate." } });
    expect(seen.maxTurns).toBe(1);
    expect(seen.tools).toEqual([]);
    expect(String(seen.systemPrompt)).toContain('You draft "output styles" for Claude Code.');
  });

  it("say so when the draft cannot be read", async () => {
    const queryFn: QueryFn = () => (async function* () {
      yield { type: "result", result: "Sorry." };
    })() as never;
    expect(await draftStyle(queryFn, "x", undefined)).toEqual({ ok: false, reason: "Claude's draft could not be read. Try again." });
  });

  it("save a drafted style and say so under the SDK's plugin name", async () => {
    const dir = join(await scratch(), "output-styles");
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () => (async function* () {
      yield { type: "result", result: '{"name":"Pirate talk","description":"Arr.","body":"Respond like a pirate."}' };
    })() as never;
    const session = new SidecarSession(queryFn, (e) => events.push(e), { stylesDir: dir });
    await session.handle({ type: "create_output_style", id: "1", description: "like a pirate", name: null });
    expect(events).toEqual([{ type: "output_style_created", name: "forge-styles:Pirate talk" }]);
    expect(await readdir(dir)).toEqual(["Pirate talk.md"]);
    await session.handle({ type: "create_output_style", id: "2", description: "like a pirate", name: "bad/name" });
    expect(events.at(-1)).toEqual({ type: "output_style_create_failed", reason: "Use letters, numbers, spaces and hyphens only." });
  });
});
