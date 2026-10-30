#!/usr/bin/env node
// Runs the fixed prompts (scripts/harness/ui_eval_prompts.json) through the real Claude Agent SDK with the forge_ui
// tools, and writes what each prompt did to <out>/<id>.json: whether the model chose a page or text, whether it
// called ui_guide first, the page arguments, the closing text, the cost. A real run uses your Claude login and costs
// tokens, so it is run on request:
//
//   cd sidecar && npm run build
//   node scripts/eval_ui.mjs --out /tmp/forge-eval/run1 --model claude-sonnet-5-5 --limit 4
//   python3 ../scripts/harness/ui_lint.py report /tmp/forge-eval/run1
//   FORGE_EVAL_DIR=/tmp/forge-eval/run1 FORGE_HARNESS_EXTRA=scripts/harness/add_eval_samples.py scripts/harness/launch.sh
//
// Flags: --out DIR (required), --model ID, --limit N, --ids a,b, --max-turns N.
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { query } from "@anthropic-ai/claude-agent-sdk";
import { createGenerativeUiServer, GENERATE_UI_TOOL, UI_GUIDE_TOOL, UI_SERVER_NAME } from "../dist/generative_ui.js";

const args = new Map();
for (let i = 2; i < process.argv.length; i += 2) args.set(process.argv[i].replace(/^--/, ""), process.argv[i + 1]);
const out = args.get("out");
if (!out) {
  console.error("usage: node scripts/eval_ui.mjs --out DIR [--model ID] [--limit N] [--ids a,b] [--max-turns N]");
  process.exit(2);
}
const promptsFile = fileURLToPath(new URL("../../scripts/harness/ui_eval_prompts.json", import.meta.url));
let prompts = JSON.parse(await readFile(promptsFile, "utf8"));
if (args.has("ids")) prompts = prompts.filter((p) => args.get("ids").split(",").includes(p.id));
if (args.has("limit")) prompts = prompts.slice(0, Number(args.get("limit")));
const model = args.get("model") ?? undefined;
const maxTurns = Number(args.get("max-turns") ?? 6);
await mkdir(out, { recursive: true });

for (const item of prompts) {
  const cwd = await mkdtemp(join(tmpdir(), "forge-eval-"));
  const run = { id: item.id, prompt: item.prompt, expect: item.expect, model: model ?? "default", called_guide: false, guide_topics: [], pages: [], text: "", tool_errors: [], cost_usd: null, duration_ms: null, error: null };
  const started = Date.now();
  try {
    const stream = query({
      prompt: item.prompt,
      options: {
        cwd,
        model,
        maxTurns,
        settingSources: [],
        mcpServers: { [UI_SERVER_NAME]: createGenerativeUiServer() },
        allowedTools: [UI_GUIDE_TOOL, GENERATE_UI_TOOL],
        // Anything else (files, shell, web) is refused: the evaluation is only about the answer.
        canUseTool: async () => ({ behavior: "deny", message: "Not available in this evaluation." }),
      },
    });
    for await (const message of stream) {
      if (message.type === "assistant") {
        for (const block of message.message.content) {
          if (block.type === "tool_use" && block.name === UI_GUIDE_TOOL) {
            run.called_guide = true;
            run.guide_topics.push(block.input?.topic);
          } else if (block.type === "tool_use" && block.name === GENERATE_UI_TOOL) {
            run.pages.push(block.input);
          } else if (block.type === "text") {
            run.text += block.text;
          }
        }
      } else if (message.type === "user" && Array.isArray(message.message?.content)) {
        for (const block of message.message.content) {
          if (block.type === "tool_result" && block.is_error) run.tool_errors.push(typeof block.content === "string" ? block.content : JSON.stringify(block.content));
        }
      } else if (message.type === "result") {
        run.cost_usd = message.total_cost_usd ?? null;
        if (message.is_error) run.error = String(message.result ?? "error");
      }
    }
  } catch (err) {
    run.error = String(err?.message ?? err);
  }
  run.duration_ms = Date.now() - started;
  await writeFile(join(out, `${item.id}.json`), JSON.stringify(run, null, 2));
  await rm(cwd, { recursive: true, force: true });
  console.log(`${item.id}: ${run.pages.length ? "page" : "text"} (expected ${item.expect}), guide=${run.called_guide}${run.error ? ", error: " + run.error.slice(0, 120) : ""}`);
}
