// Installs and checks the agent-desktop command line for the Forge app, with no npm needed: the npm
// package is one self-contained folder (the native binaries are inside), so it is downloaded from the
// registry, checked against the registry's integrity hash and unpacked under Forge's tools folder.
//
//   node install_agent_desktop.js status  <toolsDir>
//   node install_agent_desktop.js install <toolsDir>
//
// Both print one JSON line: {ok: true, installed: boolean, path?, version?} or {ok: false, message}.
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, openSync, readSync, closeSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";
import { BINARY_NAMES, findBinary, TOOLS_SUBDIR } from "./agent_desktop.js";

const run = promisify(execFile);

const REGISTRY_URL = "https://registry.npmjs.org/agent-desktop/latest";
/** The package is about 30 MB; a tarball far beyond that is not the package. */
const MAX_TARBALL_BYTES = 200 * 1024 * 1024;

export type ToolStatus = { installed: false } | { installed: true; path: string; version: string };

export function installDir(toolsDir: string): string {
  return join(toolsDir, TOOLS_SUBDIR);
}

/** `sha512-<base64>` as the registry writes it. */
export function integrityMatches(data: Uint8Array, integrity: string): boolean {
  const match = /^sha512-(.+)$/.exec(integrity);
  if (!match) return false;
  return createHash("sha512").update(data).digest("base64") === match[1];
}

/** The npm package starts with a Node launcher; only the native binary runs without `node` on the PATH. */
function isLauncher(path: string): boolean {
  const fd = openSync(path, "r");
  try {
    const head = Buffer.alloc(2);
    readSync(fd, head, 0, 2, 0);
    return head.toString() === "#!";
  } finally {
    closeSync(fd);
  }
}

async function versionOf(binary: string): Promise<string | undefined> {
  try {
    const { stdout } = await run(binary, ["version"], { timeout: 15_000 });
    const envelope = JSON.parse(stdout) as { ok?: boolean; data?: { version?: string } };
    return envelope.ok ? envelope.data?.version : undefined;
  } catch {
    return undefined;
  }
}

export async function status(toolsDir: string): Promise<ToolStatus> {
  const binary = findBinary(process.env, existsSync, toolsDir);
  if (!binary) return { installed: false };
  const version = await versionOf(binary);
  return version ? { installed: true, path: binary, version } : { installed: false };
}

async function download(url: string): Promise<Uint8Array> {
  const response = await fetch(url, { signal: AbortSignal.timeout(120_000) });
  if (!response.ok) throw new Error(`Download failed: ${url} answered ${response.status}.`);
  const data = new Uint8Array(await response.arrayBuffer());
  if (data.length > MAX_TARBALL_BYTES) throw new Error("The download is larger than expected; it was not used.");
  return data;
}

export async function install(toolsDir: string): Promise<ToolStatus> {
  const meta = (await (await fetch(REGISTRY_URL, { signal: AbortSignal.timeout(30_000) })).json()) as {
    dist?: { tarball?: string; integrity?: string };
  };
  const tarball = meta.dist?.tarball;
  const integrity = meta.dist?.integrity;
  if (!tarball || !integrity || !tarball.startsWith("https://registry.npmjs.org/")) {
    throw new Error("The registry did not describe the package as expected.");
  }
  const data = await download(tarball);
  if (!integrityMatches(data, integrity)) throw new Error("The downloaded package does not match the registry's checksum; it was not installed.");

  mkdirSync(toolsDir, { recursive: true });
  const stage = mkdtempSync(join(toolsDir, ".install-"));
  try {
    const archive = join(stage, "package.tgz");
    writeFileSync(archive, data);
    const out = join(stage, "out");
    mkdirSync(out);
    // tar ships with macOS, Linux and Windows 10 and later.
    await run("tar", ["-xzf", archive, "-C", out], { timeout: 60_000 });
    // The package's own postinstall swaps its Node launcher for the native binary only when it sits in a
    // node_modules folder, so it is unpacked as one: <tools>/agent-desktop/node_modules/agent-desktop.
    const pkg = join(out, "node_modules", TOOLS_SUBDIR);
    mkdirSync(join(out, "node_modules"));
    renameSync(join(out, "package"), pkg);
    const postinstall = join(pkg, "scripts", "postinstall.js");
    if (!existsSync(postinstall)) throw new Error("The package has no postinstall script; it was not installed.");
    await run(process.execPath, [postinstall], { cwd: pkg, timeout: 120_000 });
    const unpacked = BINARY_NAMES.map((name) => join(pkg, "bin", name)).find((path) => existsSync(path));
    if (!unpacked || isLauncher(unpacked) || !(await versionOf(unpacked))) throw new Error("The unpacked program did not run on its own; it was not installed.");

    // The new copy ran, so it replaces the old one.
    const target = installDir(toolsDir);
    rmSync(target, { recursive: true, force: true });
    renameSync(out, target);
  } finally {
    rmSync(stage, { recursive: true, force: true });
  }
  const done = await status(toolsDir);
  if (!done.installed) throw new Error("The program was installed but did not start.");
  return done;
}

async function main(argv: string[]): Promise<void> {
  const [command, toolsDir] = argv;
  try {
    if (!toolsDir || (command !== "status" && command !== "install")) throw new Error("usage: install_agent_desktop <status|install> <toolsDir>");
    const result = command === "status" ? await status(toolsDir) : await install(toolsDir);
    process.stdout.write(`${JSON.stringify({ ok: true, ...result })}\n`);
  } catch (error) {
    process.stdout.write(`${JSON.stringify({ ok: false, message: error instanceof Error ? error.message : String(error) })}\n`);
    process.exitCode = 1;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  void main(process.argv.slice(2));
}
