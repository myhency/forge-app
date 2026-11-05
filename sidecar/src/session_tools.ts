import { createSdkMcpServer, tool } from "@anthropic-ai/claude-agent-sdk";
import type { McpSdkServerConfigWithInstance, SdkMcpToolDefinition } from "@anthropic-ai/claude-agent-sdk";
import { z } from "zod";

/** The desktop app's server for session-level tools: the same name, so a session made there shows its
 *  chapters here and the model sees the same tool. */
export const SESSION_SERVER_NAME = "ccd_session";
export const MARK_CHAPTER_TOOL = `mcp__${SESSION_SERVER_NAME}__mark_chapter`;

// The desktop app's own description (read from its tool list, 2026-11-06).
const MARK_CHAPTER_DESCRIPTION = [
  "Mark the start of a new chapter in this session.",
  "Call this when the work shifts to a meaningfully different phase — e.g. after finishing exploration and starting implementation, after a fix lands and you move to verification, or when the user pivots to an unrelated request. The user sees a divider in the transcript and a floating table of contents for jumping between chapters.",
  "Use sparingly: a chapter should cover a coherent stretch of work, not every tool call. A typical session has 3–8 chapters. Do not mark a chapter for the very first message — the session start is implicit.",
  'The title is a short noun phrase ("Codebase exploration", "Auth bug fix", "Test verification"), not a sentence.',
].join("\n\n");

export function sessionToolDefinitions(): SdkMcpToolDefinition<any>[] {
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
  return [markChapter];
}

export function createSessionToolsServer(): McpSdkServerConfigWithInstance {
  return createSdkMcpServer({ name: SESSION_SERVER_NAME, tools: sessionToolDefinitions(), alwaysLoad: true });
}
