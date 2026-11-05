import { describe, expect, it } from "vitest";
import { MARK_CHAPTER_TOOL, sessionToolDefinitions, START_SESSION_TOOL, unrelatedTaskFrom, unrelatedTaskPrompt } from "../src/session_tools.js";

describe("session tools", () => {
  it("offer mark_chapter and start_session under the desktop app's names", () => {
    expect(MARK_CHAPTER_TOOL).toBe("mcp__ccd_session__mark_chapter");
    expect(START_SESSION_TOOL).toBe("mcp__ccd_session__start_session");
    expect(sessionToolDefinitions().map((t) => t.name)).toEqual(["mark_chapter", "start_session"]);
  });

  it("accept a titled chapter and refuse a blank one", async () => {
    const [mark] = sessionToolDefinitions();
    const ok = await mark.handler({ title: "Test run", summary: "pytest" }, {});
    expect(ok.isError).toBeUndefined();
    const blank = await mark.handler({ title: "  " }, {});
    expect(blank.isError).toBe(true);
  });

  it("answer a proposal without starting anything and refuse one without a prompt", async () => {
    const start = sessionToolDefinitions()[1];
    const ok = await start.handler({ initiation: "own_initiative", title: "Fix stats", prompt: "Fix mean in stats.py" }, {});
    expect(ok.isError).toBeUndefined();
    expect(ok.content[0].text).toContain("Nothing runs until");
    const empty = await start.handler({ title: "Fix", prompt: " " }, {});
    expect(empty.isError).toBe(true);
  });

  it("tell Claude about unrelated requests in the system prompt, by setting", async () => {
    expect(unrelatedTaskPrompt("ask")).toContain('initiation "offer"');
    expect(unrelatedTaskPrompt("ask")).toContain("card");
    expect(unrelatedTaskPrompt("new_session")).toContain("starts the new session");
    expect(unrelatedTaskPrompt("here")).toBe("");
    expect(unrelatedTaskFrom("nope")).toBe("ask");
  });

  it("answer a title-only offer by the setting", async () => {
    const offer = { initiation: "offer", title: "Write fib.py" };
    const started = await sessionToolDefinitions("new_session")[1].handler(offer, {});
    expect(started.content[0].text).toMatch(/^Started "Write fib.py"/);
    const card = await sessionToolDefinitions("ask")[1].handler(offer, {});
    expect(card.content[0].text).toContain("Shown to the user as a card");
    const ownWithoutPrompt = await sessionToolDefinitions("ask")[1].handler({ initiation: "own_initiative", title: "x" }, {});
    expect(ownWithoutPrompt.isError).toBe(true);
  });
});
