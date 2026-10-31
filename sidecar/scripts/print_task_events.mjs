// Prints the task_* and background_tasks_changed messages the real SDK sends while a small
// workflow runs, to see whether `task_progress` comes for a workflow and what the notification's
// `usage` holds. It sends ONE message to the model with the person's account: ask first.
//   node scripts/print_task_events.mjs /private/tmp/forge-observe
import { query } from "@anthropic-ai/claude-agent-sdk";

const cwd = process.argv[2];
if (!cwd) {
  console.error("usage: node scripts/print_task_events.mjs <throwaway folder>");
  process.exit(2);
}
const controller = new AbortController();
async function* prompt() {
  yield {
    type: "user",
    message: {
      role: "user",
      content:
        "Run a dynamic workflow named probe with the Workflow tool: phase Analyze, two parallel agents, one answers 'one' and one answers 'two' (no tools). Then say 'launched' and stop.",
    },
    parent_tool_use_id: null,
  };
  await new Promise((resolve) => controller.signal.addEventListener("abort", resolve));
}
const handle = query({
  prompt: prompt(),
  options: {
    cwd,
    abortController: controller,
    settingSources: [],
    settings: { enableWorkflows: true },
    permissionMode: "bypassPermissions",
    allowDangerouslySkipPermissions: true,
  },
});
const timer = setTimeout(() => controller.abort(), 240000);
try {
  for await (const message of handle) {
    if (message.type !== "system") continue;
    if (String(message.subtype).startsWith("task_") || message.subtype === "background_tasks_changed") {
      console.log(JSON.stringify(message));
    }
    if (message.subtype === "task_notification") {
      controller.abort();
    }
  }
} catch (error) {
  if (!controller.signal.aborted) console.error(String(error));
} finally {
  clearTimeout(timer);
}
