import { execFile } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, join } from "node:path";
import { createSdkMcpServer, tool } from "@anthropic-ai/claude-agent-sdk";
import type { McpSdkServerConfigWithInstance, SdkMcpToolDefinition } from "@anthropic-ai/claude-agent-sdk";
import { z } from "zod";

/** Not in allowedTools on purpose: the tool drives the person's whole desktop, so every call asks first. */
export const DESKTOP_SERVER_NAME = "forge_desktop";
export const DESKTOP_TOOL = `mcp__${DESKTOP_SERVER_NAME}__agent_desktop`;

const BINARY = process.platform === "win32" ? "agent-desktop.cmd" : "agent-desktop";
/** What the installer's unpacked package may hold (the Windows one is a .exe). */
export const BINARY_NAMES = ["agent-desktop", "agent-desktop.exe"];
/** Under Forge's tools folder (`FORGE_TOOLS_DIR`, set by the app) the installer unpacks the npm package here, as a node_modules folder (its postinstall needs that). */
export const TOOLS_SUBDIR = "agent-desktop";

/** The binary the installer put under the tools folder, if it is there. */
export function toolsBinary(toolsDir: string | undefined, exists: (path: string) => boolean = existsSync): string | undefined {
  if (!toolsDir) return undefined;
  const bin = join(toolsDir, TOOLS_SUBDIR, "node_modules", TOOLS_SUBDIR, "bin");
  return BINARY_NAMES.map((name) => join(bin, name)).find((path) => exists(path));
}
const TIMEOUT_MS = 60_000;
const MAX_OUTPUT_CHARS = 100_000;

const DESCRIPTION = [
  "Read and operate other desktop apps through the operating system's accessibility tree (the agent-desktop command line). Use it only when the task needs an app that has no tool of its own: a Finder window, System Settings, a native app. Never use it for a web page, a file or a command you can reach another way.",
  "args is the argument list after the command name, one item per word, no shell: ['snapshot', '--app', 'Finder', '-i'] or ['click', '@s8f3k2p9:e8']. Every call prints one JSON object with ok, data or error. Read error.suggestion and error.disposition before retrying; retry only when the disposition says it is safe.",
  "Work in a loop: snapshot the app (-i lists only what can be pressed), act on one ref, then snapshot again to check. A ref belongs to the snapshot that made it; STALE_REF or AMBIGUOUS_TARGET means the screen changed, so snapshot again. With several windows pass --window-id (from list-windows).",
  "Actions run headless by default and do not move the cursor. Text fields in web views often ignore 'type' and 'set-value' (ACTION_FAILED, value stays empty): retry that one with --headed, which sends real key events and may move focus.",
  "Call ['skills', 'get', 'desktop'] once to read the full command guide, and ['<command>', '--help'] for one command. Snapshots can hold private text: do not copy them into files or messages beyond what the task needs. Do not press keys or click anything the person did not ask for.",
].join("\n\n");

const ALWAYS_LOADED = { alwaysLoad: true };

function answer(text: string, isError = false) {
  return { content: [{ type: "text" as const, text }], ...(isError ? { isError: true } : {}) };
}

/** A GUI app starts with a short PATH, so the usual places of a global npm, Homebrew or Bun install are added. */
export function searchDirs(env: NodeJS.ProcessEnv = process.env): string[] {
  const home = env.HOME ?? env.USERPROFILE ?? homedir();
  const dirs = (env.PATH ?? "").split(delimiter).filter(Boolean);
  dirs.push("/opt/homebrew/bin", "/usr/local/bin", join(home, ".npm-global", "bin"), join(home, ".bun", "bin"), join(home, ".local", "bin"));
  if (env.APPDATA) dirs.push(join(env.APPDATA, "npm"));
  return [...new Set(dirs)];
}

/** Order: an explicit AGENT_DESKTOP_BIN, the copy the app installed, then the PATH and the usual global install places. */
export function findBinary(
  env: NodeJS.ProcessEnv = process.env,
  exists: (path: string) => boolean = existsSync,
  toolsDir: string | undefined = env.FORGE_TOOLS_DIR,
): string | undefined {
  if (env.AGENT_DESKTOP_BIN && exists(env.AGENT_DESKTOP_BIN)) return env.AGENT_DESKTOP_BIN;
  const installed = toolsBinary(toolsDir, exists);
  if (installed) return installed;
  for (const dir of searchDirs(env)) {
    const candidate = join(dir, BINARY);
    if (exists(candidate)) return candidate;
  }
  return undefined;
}

export type RunResult = { stdout: string; stderr: string; code: number | null; timedOut: boolean };
export type Runner = (binary: string, args: string[]) => Promise<RunResult>;

const run: Runner = (binary, args) =>
  new Promise((resolve) => {
    execFile(binary, args, { timeout: TIMEOUT_MS, maxBuffer: 16 * 1024 * 1024, env: { ...process.env, PATH: searchDirs().join(delimiter) } }, (error, stdout, stderr) => {
      const failure = error as (NodeJS.ErrnoException & { killed?: boolean }) | null;
      resolve({ stdout, stderr, code: failure ? (typeof failure.code === "number" ? failure.code : 1) : 0, timedOut: failure?.killed === true });
    });
  });

export const INSTALL_HINT =
  "agent-desktop is not installed. Tell the person to open Settings > Code > Control other apps and press Install (or run `npm install -g agent-desktop`), then try again. On macOS it also needs Accessibility permission for Forge.";

export function desktopToolDefinitions(
  options: { find?: () => string | undefined; runner?: Runner } = {},
): SdkMcpToolDefinition<any>[] {
  const find = options.find ?? (() => findBinary());
  const runner = options.runner ?? run;
  const agentDesktop = tool(
    "agent_desktop",
    DESCRIPTION,
    { args: z.array(z.string().max(4_000)).min(1).max(64) },
    async ({ args }) => {
      const binary = find();
      if (!binary) return answer(INSTALL_HINT, true);
      const done = await runner(binary, args);
      if (done.timedOut) return answer(`agent-desktop did not finish in ${TIMEOUT_MS / 1000} seconds and was stopped.`, true);
      // The CLI prints its JSON envelope (ok:false included) on stdout; stderr only matters when stdout is empty.
      let out = done.stdout.trim() || done.stderr.trim() || `(no output, exit code ${done.code})`;
      if (out.length > MAX_OUTPUT_CHARS) {
        out = `${out.slice(0, MAX_OUTPUT_CHARS)}\n[cut at ${MAX_OUTPUT_CHARS} characters: snapshot a smaller part with --root, --max-depth or -i]`;
      }
      return answer(out, done.code !== 0);
    },
    ALWAYS_LOADED,
  );
  return [agentDesktop];
}

export function createDesktopServer(): McpSdkServerConfigWithInstance {
  return createSdkMcpServer({ name: DESKTOP_SERVER_NAME, tools: desktopToolDefinitions(), alwaysLoad: true });
}
