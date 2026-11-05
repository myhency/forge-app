import { createSdkMcpServer, tool } from "@anthropic-ai/claude-agent-sdk";
import type { McpSdkServerConfigWithInstance, SdkMcpToolDefinition } from "@anthropic-ai/claude-agent-sdk";
import { z } from "zod";

/** The desktop app's server for session-level tools: the same name, so a session made there shows its
 *  chapters here and the model sees the same tool. */
export const SESSION_SERVER_NAME = "ccd_session";
export const MARK_CHAPTER_TOOL = `mcp__${SESSION_SERVER_NAME}__mark_chapter`;
export const START_SESSION_TOOL = `mcp__${SESSION_SERVER_NAME}__start_session`;
export const DISMISS_TASK_TOOL = `mcp__${SESSION_SERVER_NAME}__dismiss_task`;
export const HAND_OFF_TOOL = `mcp__${SESSION_SERVER_NAME}__hand_off_to_session`;

/** A proposal's handle, as the desktop app gives one (`task_ab12cd34`): dismiss_task names it. */
function newTaskId(): string {
  return `task_${Math.random().toString(16).slice(2, 10).padEnd(8, "0")}`;
}

// The desktop app's own description (read from its tool list, 2026-11-06).
const MARK_CHAPTER_DESCRIPTION = [
  "Mark the start of a new chapter in this session.",
  "Call this when the work shifts to a meaningfully different phase — e.g. after finishing exploration and starting implementation, after a fix lands and you move to verification, or when the user pivots to an unrelated request. The user sees a divider in the transcript and a floating table of contents for jumping between chapters.",
  "Use sparingly: a chapter should cover a coherent stretch of work, not every tool call. A typical session has 3–8 chapters. Do not mark a chapter for the very first message — the session start is implicit.",
  'The title is a short noun phrase ("Codebase exploration", "Auth bug fix", "Test verification"), not a sentence.',
].join("\n\n");

// Forge's shorter take on the desktop app's start_session: every call is a proposal the person starts.
const START_SESSION_DESCRIPTION = [
  "Propose a separate Claude Code session for follow-up work, while you stay with the user.",
  "Use it for work you noticed that does not belong in the current change — a second bug, stale docs, missing tests, a refactor the current task turned up — or when the user asks you to split something off into its own session. Do not use it for the task the user asked you to do here, or for small things you can finish inline.",
  "The user sees a card with your title and the first line of your prompt, and nothing runs until they press Start (the new session opens beside this one, in the same folder, with your prompt as its first message) or choose to do it here. Your current turn continues: mention the card in one short sentence, do not wait for it.",
  "prompt is the brief and must stand alone: the task, the files or area, and what done looks like. background (optional, at least a few sentences) is what the new session needs to know from this conversation; it is sent before the prompt.",
].join("\n\n");

/** The desktop app's `관련 없는 작업을 요청할 때` setting: what to do when the user asks for something
 *  unrelated to this session's work. */
export type UnrelatedTask = "ask" | "here" | "new_session";

/** The system prompt section that tells Claude how to treat unrelated work, after the desktop app's
 *  own (read from a desktop session's request, 2026-11-06): a request is a "new task" when a fresh
 *  session would do it as well from the user's words alone; such a turn starts with a title-only
 *  `start_session` offer and nothing else, and the app does the rest (a card, or a session started at
 *  once). With `항상 여기서 실행` there is no section: the work stays here. */
export function unrelatedTaskPrompt(unrelated: UnrelatedTask): string {
  if (unrelated === "here") return "";
  return [
    "# Unrelated requests (this app)",
    `Before you start on a request, decide what it is to this session. Until you have finished a task here, it is the session's first task. After that, a request is a "new task" when it is unrelated work: a fresh session would do it just as well from the user's words alone, because it builds on nothing done, found or decided in this conversation. Everything else stays here: corrections, follow-ups, questions, the next iteration of the work here, going ahead with what you just investigated or suggested, and anything a few tool calls settle. When you are unsure, it stays here.`,
    `A new-task turn, and any turn where the user names a task and asks you to split it off or make a separate session for it, does not start on the work: its first and only tool call is \`${START_SESSION_TOOL}\` with initiation "offer" and a title, no prompt or background, so the app can act on it at once. ${
      unrelated === "new_session"
        ? "The app then starts the new session with the user's words; say so in one short sentence and end the turn."
        : "The app then shows the user a card to start it in a new session or do it here; say so in one short sentence and end the turn."
    }`,
  ].join("\n\n");
}

export function unrelatedTaskFrom(value: unknown): UnrelatedTask {
  return value === "here" || value === "new_session" ? value : "ask";
}

export function sessionToolDefinitions(unrelated: UnrelatedTask = "ask"): SdkMcpToolDefinition<any>[] {
  const markChapter = tool(
    "mark_chapter",
    MARK_CHAPTER_DESCRIPTION,
    {
      title: z.string().describe("Short noun-phrase title for the chapter (under 40 chars). Shown in the table of contents."),
      summary: z.string().optional().describe("Optional one-line summary of what this chapter covers. Shown on hover in the table of contents."),
    },
    async (args) => {
      const title = args.title.trim();
      if (title === "") {
        return { content: [{ type: "text" as const, text: "A chapter needs a title." }], isError: true };
      }
      return { content: [{ type: "text" as const, text: `Chapter marked: ${title}` }] };
    },
    { alwaysLoad: true },
  );
  const startSession = tool(
    "start_session",
    START_SESSION_DESCRIPTION,
    {
      initiation: z.enum(["user_asked", "own_initiative", "offer"]).optional().describe('"own_initiative" for work you propose yourself; "user_asked" or "offer" when the user asked for a separate session. Every kind is shown as a card the user starts.'),
      title: z.string().describe("Under 60 chars; an imperative action phrase naming the task. Shown on the card and as the new session's first line."),
      prompt: z.string().optional().describe('The brief: the task, the files or area, what done looks like. Self-contained. Left out only for an "offer" of the user\'s own unrelated request, which starts with their words.'),
      background: z.string().optional().describe("What the new session needs to know from this conversation; sent before the prompt."),
      context: z.enum(["fresh", "fork"]).optional().describe('Only "fresh" is supported here: the new session gets your background and prompt only.'),
    },
    async (args) => {
      const prompt = (args.prompt ?? "").trim();
      if (args.title.trim() === "" || (prompt === "" && args.initiation !== "offer")) {
        return { content: [{ type: "text" as const, text: "A proposal needs a title and a prompt." }], isError: true };
      }
      const title = args.title.trim();
      if (prompt === "" && unrelated === "new_session") {
        return { content: [{ type: "text" as const, text: `Started "${title}" in a new session with the user's words. Say so in one short sentence and end your turn; do not do the work here.` }] };
      }
      const taskId = newTaskId();
      if (prompt === "") {
        return { content: [{ type: "text" as const, text: `Shown to the user as a card "${title}": they start it in a new session or have you do it here. Say so in one short sentence and end your turn; do not start the work. (task_id: ${taskId})` }] };
      }
      return {
        content: [
          {
            type: "text" as const,
            text: `Shown to the user as a card "${title}". Nothing runs until they press Start or choose to do it here; carry on with your answer. (task_id: ${taskId})`,
          },
        ],
      };
    },
    { alwaysLoad: true },
  );
  // The desktop app's own description (loaded through a desktop session, 2026-11-06).
  const dismissTask = tool(
    "dismiss_task",
    [
      'Withdraw a proposal you made with start_session (initiation "own_initiative" or "offer") that the user has not started.',
      'Call this when it went stale: the work was done here after all (by you or the user), it turned out unnecessary, or you proposed a better-scoped replacement (propose the new one first, then dismiss the old task_id). A card the user answered has already left, "Do it here" included, so it needs no call.',
    ].join("\n\n"),
    {
      task_id: z.string().describe("The task_id the start_session proposal returned."),
      reason: z.string().optional().describe('Optional one-line reason the suggestion is no longer needed, e.g. "fixed in this session" or "superseded by task_ab12cd34".'),
    },
    async (args) => {
      if (!/^task_[0-9a-f]{8}$/.test(args.task_id.trim())) {
        return { content: [{ type: "text" as const, text: "No proposal has that task_id." }], isError: true };
      }
      return { content: [{ type: "text" as const, text: `Withdrew ${args.task_id.trim()}: its card is gone.` }] };
    },
    { alwaysLoad: true },
  );
  // The desktop app's own description (loaded through a desktop session, 2026-11-06); Forge has no
  // worktree branch to cut from, so the fork is a copy of this conversation in the same folder.
  const handOff = tool(
    "hand_off_to_session",
    "Move the task you are in the middle of into its own session (a fork of this conversation plus your remaining plan) so you can answer the user now. Use it when a new user message about something else arrives mid-task — not for small corrections or when the user said to keep the work here. Bold title line and one sentence right before the call; after it, stop that task here and answer the new message — no recap.",
    {
      title: z.string().describe("Under 60 chars; names the task being moved."),
      remaining_plan: z.string().describe("What is left to do: steps, files, what done looks like."),
    },
    async (args) => {
      if (args.title.trim() === "" || args.remaining_plan.trim() === "") {
        return { content: [{ type: "text" as const, text: "A hand-off needs a title and the remaining plan." }], isError: true };
      }
      return { content: [{ type: "text" as const, text: `Handed off "${args.title.trim()}": a fork of this conversation carries on with your plan beside this session. Stop that task here and answer the new message — no recap.` }] };
    },
    { alwaysLoad: true },
  );
  return [markChapter, startSession, dismissTask, handOff];
}

export function createSessionToolsServer(unrelated: UnrelatedTask = "ask"): McpSdkServerConfigWithInstance {
  return createSdkMcpServer({ name: SESSION_SERVER_NAME, tools: sessionToolDefinitions(unrelated), alwaysLoad: true });
}
