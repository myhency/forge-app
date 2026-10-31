// Prints, for each output style name Forge offers, the style the real SDK reports in its init
// message, and the styles it lists as available. Opens a query per style and closes it without
// sending a message to the model.
import { query } from "@anthropic-ai/claude-agent-sdk";

const names = [undefined, "Concise", "Explanatory", "Learning", "Proactive"];

for (const name of names) {
  const controller = new AbortController();
  async function* never() {
    await new Promise((resolve) => controller.signal.addEventListener("abort", resolve));
  }
  const handle = query({
    prompt: never(),
    options: {
      cwd: process.cwd(),
      abortController: controller,
      settingSources: ["project"],
      ...(name ? { settings: { outputStyle: name } } : {}),
    },
  });
  try {
    const init = await Promise.race([
      handle.initializationResult(),
      new Promise((_, reject) => setTimeout(() => reject(new Error("no answer in 60 s")), 60000)),
    ]);
    console.log(JSON.stringify({ asked: name ?? "(none)", output_style: init.output_style ?? null, available: init.available_output_styles ?? null }));
  } catch (error) {
    console.log(JSON.stringify({ asked: name ?? "(none)", error: String(error) }));
  } finally {
    controller.abort();
  }
}
process.exit(0);
