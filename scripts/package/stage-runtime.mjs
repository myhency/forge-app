#!/usr/bin/env node
// Gathers what a packaged Forge needs beside its program, so nothing has to be installed on the PC:
//   <out>/node[.exe]            the Node the sidecar runs on (the version pinned in sidecar/package.json)
//   <out>/sidecar/dist/...      the built sidecar
//   <out>/sidecar/node_modules  its production dependencies, with the Claude binary of the TARGET platform
//
//   node scripts/package/stage-runtime.mjs --platform darwin --arch arm64
//   node scripts/package/stage-runtime.mjs --platform win32 --arch x64 --out crates/forge-app/runtime
//
// Options: --platform darwin|win32 (default: this machine)   --arch arm64|x64 (default: this machine)
//          --out <dir> (default crates/forge-app/runtime)    --node-archive <file> (a Node archive already downloaded)
//          --mirror <url> (default https://nodejs.org/dist; an internal mirror works the same)
// Runs on macOS and Windows (it unpacks a .zip with the system `tar`, which is bsdtar there).
// Either platform's runtime can be staged from either.

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const sidecar = join(root, "sidecar");

export function parseArgs(argv, defaults) {
  const options = { ...defaults };
  for (let i = 0; i < argv.length; i += 2) {
    const flag = argv[i];
    const value = argv[i + 1];
    if (!flag.startsWith("--") || value === undefined) throw new Error(`Expected "--name value", got "${flag}"`);
    options[flag.slice(2)] = value;
  }
  return options;
}

/** The Node archive for a target and the path of the program inside it. */
export function nodeArchive(version, platform, arch) {
  if (!["arm64", "x64"].includes(arch)) throw new Error(`Unsupported arch "${arch}" (arm64 or x64)`);
  if (platform === "darwin") {
    const stem = `node-v${version}-darwin-${arch}`;
    return { file: `${stem}.tar.gz`, program: `${stem}/bin/node`, name: "node" };
  }
  if (platform === "win32") {
    const stem = `node-v${version}-win-${arch}`;
    return { file: `${stem}.zip`, program: `${stem}/node.exe`, name: "node.exe" };
  }
  throw new Error(`Unsupported platform "${platform}" (darwin or win32)`);
}

/** The sha256 that SHASUMS256.txt lists for `file`. */
export function expectedSha(shasums, file) {
  const line = shasums.split("\n").find((entry) => entry.trim().endsWith(`  ${file}`));
  if (!line) throw new Error(`${file} is not in SHASUMS256.txt`);
  return line.trim().split(/\s+/)[0];
}

const sha256 = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");

async function download(url, to) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: ${response.status}`);
  writeFileSync(to, Buffer.from(await response.arrayBuffer()));
}

const run = (command, args, cwd) => execFileSync(command, args, { cwd, stdio: "inherit", shell: process.platform === "win32" && command === "npm" });

async function stageNode(options, version, scratch, out) {
  const { file, program, name } = nodeArchive(version, options.platform, options.arch);
  const archive = options["node-archive"] ?? join(scratch, file);
  const shasums = join(scratch, "SHASUMS256.txt");
  await download(`${options.mirror}/v${version}/SHASUMS256.txt`, shasums);
  if (!options["node-archive"]) await download(`${options.mirror}/v${version}/${file}`, archive);
  const want = expectedSha(readFileSync(shasums, "utf8"), basename(file));
  if (sha256(archive) !== want) throw new Error(`${archive} does not match the sha256 in SHASUMS256.txt`);
  // `tar` unpacks .tar.gz everywhere and .zip on Windows 10+ and macOS.
  run("tar", ["-xf", archive, "-C", scratch, program]);
  cpSync(join(scratch, program), join(out, name));
}

function stageSidecar(options, out) {
  run("npm", ["ci"], sidecar);
  run("npm", ["run", "build"], sidecar);
  const target = join(out, "sidecar");
  mkdirSync(target, { recursive: true });
  cpSync(join(sidecar, "dist"), join(target, "dist"), { recursive: true });
  for (const file of ["package.json", "package-lock.json"]) cpSync(join(sidecar, file), join(target, file));
  // The SDK ships its Claude binary as an optional package per platform; --os/--cpu pick the target's.
  run("npm", ["ci", "--omit=dev", "--ignore-scripts", `--os=${options.platform}`, `--cpu=${options.arch}`], target);
  const binary = join(target, "node_modules/@anthropic-ai", `claude-agent-sdk-${options.platform}-${options.arch}`);
  if (!existsSync(binary)) throw new Error(`The Claude binary for ${options.platform}-${options.arch} was not installed (${binary})`);
}

async function main() {
  const options = parseArgs(process.argv.slice(2), {
    platform: process.platform,
    arch: process.arch,
    out: join(root, "crates/forge-app/runtime"),
    mirror: "https://nodejs.org/dist",
  });
  const version = JSON.parse(readFileSync(join(sidecar, "package.json"), "utf8")).engines.node;
  const out = resolve(options.out);
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  const scratch = mkdtempSync(join(tmpdir(), "forge-stage-"));
  try {
    await stageNode(options, version, scratch, out);
    stageSidecar(options, out);
    writeFileSync(join(out, "VERSION.json"), JSON.stringify({ node: version, platform: options.platform, arch: options.arch }, null, 2));
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
  console.log(`Staged Node ${version} and the sidecar for ${options.platform}-${options.arch} in ${out}`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error.message);
    process.exit(1);
  });
}
