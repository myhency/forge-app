// Prints the models the real SDK lists for a session: value, resolved id, name, effort levels, auto mode.
// Opens a query and closes it without sending a message.
import { query } from "@anthropic-ai/claude-agent-sdk";

const controller = new AbortController();
async function* never() {
  await new Promise((resolve) => controller.signal.addEventListener("abort", resolve));
}
const handle = query({ prompt: never(), options: { cwd: process.cwd(), abortController: controller, settingSources: [] } });
try {
  const models = await Promise.race([
    handle.supportedModels(),
    new Promise((_, reject) => setTimeout(() => reject(new Error("no answer in 60 s")), 60000)),
  ]);
  for (const m of models) {
    console.log(JSON.stringify({
      value: m.value,
      resolvedModel: m.resolvedModel ?? null,
      displayName: m.displayName,
      supportsEffort: m.supportsEffort ?? null,
      effortLevels: m.supportedEffortLevels ?? null,
      supportsAutoMode: m.supportsAutoMode ?? null,
    }));
  }
} finally {
  controller.abort();
  process.exit(0);
}
