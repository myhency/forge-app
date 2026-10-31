import { describe, expect, it } from "vitest";
import { createTurnToolsServer, turnToolDefinitions, TURN_SERVER_NAME, UPDATE_PLAN_TOOL, WRAP_UP_TOOL } from "../src/turn_tools.js";

function tools() {
  const list = turnToolDefinitions();
  const byName = (name: string) => list.find((tool) => tool.name === name)!;
  return { updatePlan: byName("update_plan"), wrapUp: byName("wrap_up") };
}

async function text(result: Promise<{ content: Array<{ type: string; text?: string }>; isError?: boolean }>) {
  const done = await result;
  return { text: done.content.map((part) => part.text ?? "").join(""), isError: done.isError === true };
}

describe("turn tools", () => {
  it("names the server and the two tools like the desktop app", () => {
    expect(TURN_SERVER_NAME).toBe("ccd_turn");
    expect(UPDATE_PLAN_TOOL).toBe("mcp__ccd_turn__update_plan");
    expect(WRAP_UP_TOOL).toBe("mcp__ccd_turn__wrap_up");
    expect(turnToolDefinitions().map((tool) => tool.name).sort()).toEqual(["update_plan", "wrap_up"]);
    expect(createTurnToolsServer().type).toBe("sdk");
  });

  it("keeps both tools out of tool search, so the model sees their descriptions from the first turn", () => {
    for (const definition of turnToolDefinitions()) {
      expect(definition._meta?.["anthropic/alwaysLoad"]).toBe(true);
    }
  });

  it("update_plan answers with the step count for a whole plan", async () => {
    const { updatePlan } = tools();
    const out = await text(
      updatePlan.handler(
        {
          scope: "first_task",
          items: [
            { content: "Read the code", status: "in_progress" },
            { content: "Fix it", status: "pending" },
          ],
        },
        {},
      ),
    );
    expect(out).toEqual({ text: "Plan shown (2 steps).", isError: false });
  });

  it("update_plan accepts advance, done and start once a plan exists", async () => {
    const { updatePlan } = tools();
    await updatePlan.handler({ items: [{ content: "a", status: "in_progress" }, { content: "b", status: "pending" }, { content: "c", status: "pending" }] }, {});
    expect((await text(updatePlan.handler({ advance: true }, {}))).isError).toBe(false);
    expect((await text(updatePlan.handler({ done: [1], start: 3 }, {}))).isError).toBe(false);
  });

  it("update_plan refuses an empty call, an update before any plan and a number outside the list", async () => {
    const { updatePlan } = tools();
    expect((await text(updatePlan.handler({}, {}))).isError).toBe(true);
    const early = await text(updatePlan.handler({ advance: true }, {}));
    expect(early.isError).toBe(true);
    expect(early.text).toContain("items");
    await updatePlan.handler({ items: [{ content: "a", status: "in_progress" }] }, {});
    const far = await text(updatePlan.handler({ done: [2] }, {}));
    expect(far.isError).toBe(true);
    expect(far.text).toContain("1");
  });

  it("each tools instance keeps its own plan", async () => {
    const first = tools().updatePlan;
    const second = tools().updatePlan;
    await first.handler({ items: [{ content: "a", status: "in_progress" }] }, {});
    expect((await text(second.handler({ advance: true }, {}))).isError).toBe(true);
  });

  it("wrap_up says the closing message is still needed, and that the box asks the question only when there is one", async () => {
    const { wrapUp } = tools();
    const plain = await text(wrapUp.handler({ recap: "Done." }, {}));
    expect(plain.isError).toBe(false);
    expect(plain.text).toContain("Your closing message is your answer to the user");
    expect(plain.text).not.toContain("The box asks your question");
    const asked = await text(wrapUp.handler({ recap: "Done.", ask: "Commit?", actions: [{ label: "Commit" }] }, {}));
    expect(asked.text).toContain("The box asks your question, so do not ask it again.");
  });
});
