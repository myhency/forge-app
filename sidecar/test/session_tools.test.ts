import { describe, expect, it } from "vitest";
import { MARK_CHAPTER_TOOL, sessionToolDefinitions } from "../src/session_tools.js";

describe("session tools", () => {
  it("offer mark_chapter under the desktop app's name", () => {
    expect(MARK_CHAPTER_TOOL).toBe("mcp__ccd_session__mark_chapter");
    expect(sessionToolDefinitions().map((t) => t.name)).toEqual(["mark_chapter"]);
  });

  it("accept a titled chapter and refuse a blank one", async () => {
    const [mark] = sessionToolDefinitions();
    const ok = await mark.handler({ title: "Test run", summary: "pytest" }, {});
    expect(ok.isError).toBeUndefined();
    const blank = await mark.handler({ title: "  " }, {});
    expect(blank.isError).toBe(true);
  });
});
