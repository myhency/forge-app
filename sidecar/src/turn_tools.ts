import { createSdkMcpServer, tool } from "@anthropic-ai/claude-agent-sdk";
import type { McpSdkServerConfigWithInstance, SdkMcpToolDefinition } from "@anthropic-ai/claude-agent-sdk";
import { z } from "zod";

/** The desktop app's server name: the model sees the same tool names, and a session made there opens here. */
export const TURN_SERVER_NAME = "ccd_turn";
export const UPDATE_PLAN_TOOL = `mcp__${TURN_SERVER_NAME}__update_plan`;
export const WRAP_UP_TOOL = `mcp__${TURN_SERVER_NAME}__wrap_up`;

const MAX_STEPS = 12;
const MAX_ACTIONS = 2;

const UPDATE_PLAN_DESCRIPTION = [
  "Show the person a task list of your plan while you work. Call it at the start of every request that will create or edit a file or run more than one command, even a small one: the person watches the list instead of waiting. Skip it only for a single-command answer or plain conversation.",
  "Send it in the same message as another tool call, never alone, so it costs no extra round trip.",
  'First call: {items: [...]}, the whole plan in order, short imperative phrases (about 7 words, no numbering), the step you are starting "in_progress", the rest "pending".',
  "To move on: {advance: true} together with the call that begins the next step (it completes the step in progress and starts the next pending one).",
  "Out of order: {done: [2], start: 4} (1-based numbers).",
  "If the plan changes, send {items} again with the whole new list.",
  "Keep exactly one step in_progress while you work, and none when everything is done. Do not also write the plan as a numbered list in your message: the task list is the plan.",
].join("\n\n");

const WRAP_UP_DESCRIPTION = [
  "Call this once at the end of every turn in which you created or edited a file or ran a command, right before your closing message. It shows the person a box: a two-or-three-sentence recap and, when the next move is theirs (for example whether to commit, run or keep going), one question with one or two reply buttons.",
  "Skip it only for short answers and plain conversation with no file or command work.",
  "Your closing message is still your full answer, written right after this call; the recap never replaces it.",
  "recap: what is now true or changed and where it stands, including what is not done or not verified (300 characters or fewer, said to the person, no recommendation).",
  "ask: only when the next move is the person's call. One short question (100 characters or fewer) that does not list the buttons' options. Without ask no box is drawn.",
  "actions: one or two reply buttons, your recommendation first. label is the button text (1-3 words, 24 characters or fewer); message is what a click sends (300 characters or fewer; leave it out when the label says it all).",
].join("\n\n");

/** Without this the SDK defers MCP tools behind tool search: the model sees only the two names, never the
 *  descriptions, and (checked with a real turn) never calls them. */
const ALWAYS_LOADED = { alwaysLoad: true };

const stepSchema = z.object({
  content: z.string(),
  status: z.enum(["pending", "in_progress", "completed"]),
});

function answer(text: string, isError = false) {
  return { content: [{ type: "text" as const, text }], ...(isError ? { isError: true } : {}) };
}

/** The two tools. Each call of this function has its own memory of the plan's length, which is all the
 *  tool keeps: the cards are drawn from the recorded calls, not from here. */
export function turnToolDefinitions(): SdkMcpToolDefinition<any>[] {
  let steps = 0;
  const updatePlan = tool(
    "update_plan",
    UPDATE_PLAN_DESCRIPTION,
    {
      scope: z.enum(["first_task", "continuation", "new_task"]).optional(),
      items: z.array(stepSchema).min(1).max(MAX_STEPS).optional(),
      advance: z.boolean().optional(),
      done: z.array(z.number()).optional(),
      start: z.number().optional(),
    },
    async (args) => {
      if (args.items) {
        steps = args.items.length;
        return answer(`Plan shown (${steps} steps).`);
      }
      if (!args.advance && !args.done && args.start === undefined) {
        return answer("Nothing to update: send {items} for a plan, or {advance}, {done} or {start} to change it.", true);
      }
      if (steps === 0) {
        return answer("There is no plan yet: send {items} first.", true);
      }
      const numbers = [...(args.done ?? []), ...(args.start === undefined ? [] : [args.start])];
      if (numbers.some((n) => !Number.isInteger(n) || n < 1 || n > steps)) {
        return answer(`Step numbers are 1 to ${steps}.`, true);
      }
      return answer(`Plan updated (${steps} steps).`);
    },
    ALWAYS_LOADED,
  );
  const wrapUp = tool(
    "wrap_up",
    WRAP_UP_DESCRIPTION,
    {
      recap: z.string().min(1),
      ask: z.string().optional(),
      blocked: z.boolean().optional(),
      actions: z
        .array(z.object({ label: z.string().optional(), message: z.string().optional() }))
        .max(MAX_ACTIONS)
        .optional(),
    },
    async (args) => {
      const base =
        "Shown in the next-step box. Your closing message is your answer to the user: write it now, in full, as the last thing you say this turn.";
      return answer(args.ask ? `${base} The box asks your question, so do not ask it again.` : base);
    },
    ALWAYS_LOADED,
  );
  return [updatePlan, wrapUp];
}

export function createTurnToolsServer(): McpSdkServerConfigWithInstance {
  return createSdkMcpServer({ name: TURN_SERVER_NAME, tools: turnToolDefinitions(), alwaysLoad: true });
}
