// Throwaway spike (Plan 56, Task 1; deleted in Task 7). Real SDK, real account, haiku, tiny turns.
// Question (a): after a plain allow of ExitPlanMode, does setPermissionMode("auto") right after leave the session in auto?
// Question (b): does allow with updatedPermissions [{type:"setMode",mode:"auto",destination:"session"}] do the same?
// "In auto" is judged by: the next Bash call does NOT reach canUseTool (in default it would ask).
import { query } from "@anthropic-ai/claude-agent-sdk";
import { mkdirSync, rmSync } from "node:fs";

async function run(variant) {
  const cwd = `/private/tmp/forge-observe/spike-plan-${variant}`;
  rmSync(cwd, { recursive: true, force: true });
  mkdirSync(cwd, { recursive: true });
  const controller = new AbortController();
  const queue = [];
  let wake = null;
  const push = (text) => {
    queue.push({ type: "user", message: { role: "user", content: text }, parent_tool_use_id: null, session_id: "" });
    wake?.();
  };
  async function* input() {
    while (!controller.signal.aborted) {
      if (queue.length) yield queue.shift();
      else await new Promise((r) => { wake = r; });
    }
  }
  const log = { variant, canUseToolCalls: [], modeSeen: [], setModeError: null, bashAsked: null };
  let handle;
  let phase = "plan";
  const canUseTool = async (name, inp, opts) => {
    log.canUseToolCalls.push(`${phase}:${name}`);
    if (name === "ExitPlanMode") {
      if (variant === "b") return { behavior: "allow", updatedInput: inp, updatedPermissions: [{ type: "setMode", mode: "auto", destination: "session" }] };
      setTimeout(async () => {
        try { await handle.setPermissionMode("auto"); } catch (e) { log.setModeError = String(e); }
      }, 0);
      return { behavior: "allow", updatedInput: inp };
    }
    if (phase === "after" && name === "Bash") log.bashAsked = true;
    return { behavior: "deny", message: "spike: denied" };
  };
  handle = query({
    prompt: input(),
    options: { cwd, model: "haiku", permissionMode: "plan", canUseTool, abortController: controller, settingSources: [], maxTurns: 6 },
  });
  push("Make a one-line plan to create a file hello.txt containing hi, then call ExitPlanMode with that plan. Do nothing else.");
  const timer = setTimeout(() => controller.abort(), 120000);
  let results = 0;
  try {
    for await (const m of handle) {
      const s = JSON.stringify(m);
      const mm = s.match(/"permissionMode":"(\w+)"/);
      if (mm) log.modeSeen.push(`${m.type}/${m.subtype ?? ""}:${mm[1]}`);
      if (m.type === "result") {
        results++;
        if (results === 1) {
          phase = "after";
          await new Promise((r) => setTimeout(r, 1500));
          push("Now run the Bash command `echo hi > hello.txt` exactly once. Nothing else.");
        } else break;
      }
    }
  } catch (e) { log.error = String(e); }
  clearTimeout(timer);
  log.bashAsked = log.bashAsked === true;
  log.inAuto = !log.bashAsked && log.canUseToolCalls.some((c) => c === "after:Bash") === false;
  controller.abort();
  return log;
}

const a = await run("a");
console.log(JSON.stringify(a));
const b = await run("b");
console.log(JSON.stringify(b));
console.log(`(a) setPermissionMode("auto") after plain allow leaves session in auto: ${!a.bashAsked && !a.setModeError && !a.error}`);
console.log(`(b) allow + updatedPermissions setMode auto leaves session in auto: ${!b.bashAsked && !b.error}`);
process.exit(0);
