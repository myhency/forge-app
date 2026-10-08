#!/usr/bin/env node
// Builds Forge for the machine it runs on, with its own Node, ready to hand to a colleague:
//   macOS:    dist/Forge.app          (also dist/Forge-macos-<arch>.zip)
//   Windows:  dist/Forge-win-x64/     (Forge.exe + runtime/, also dist/Forge-win-x64.zip)
// A program is built with the toolchain of its own platform, so each is built on that platform.
//
//   node scripts/package/build.mjs
//
// Needs: Rust, Node 22 and, on macOS, `dx` (cargo install dioxus-cli --version 0.7.10).

import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, renameSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const dist = join(root, "dist");
const runtime = join(root, "crates/forge-app/runtime");
const run = (command, args, cwd = root) => execFileSync(command, args, { cwd, stdio: "inherit" });
const stage = () => run("node", [join(root, "scripts/package/stage-runtime.mjs"), "--out", runtime]);

function buildMac() {
  const arch = process.arch;
  stage();
  run("dx", ["bundle", "--release", "--package-types", "macos"], join(root, "crates/forge-app"));
  const built = join(root, "target/dx/forge-app/bundle/macos/macos/ForgeApp.app");
  const app = join(dist, "Forge.app");
  rmSync(dist, { recursive: true, force: true });
  mkdirSync(dist, { recursive: true });
  renameSync(built, app);
  cpSync(runtime, join(app, "Contents/Resources/runtime"), { recursive: true, verbatimSymlinks: true });
  // Files were added after the bundler signed the app; sign it again (ad hoc: an internal build has no Developer ID).
  run("codesign", ["--force", "--deep", "--sign", "-", app]);
  run("ditto", ["-c", "-k", "--keepParent", app, join(dist, `Forge-macos-${arch}.zip`)]);
  console.log(`Built ${app}`);
}

function buildWindows() {
  stage();
  run("cargo", ["build", "--release", "-p", "forge-app"]);
  const folder = join(dist, "Forge-win-x64");
  rmSync(dist, { recursive: true, force: true });
  mkdirSync(folder, { recursive: true });
  cpSync(join(root, "target/release/forge-app.exe"), join(folder, "Forge.exe"));
  cpSync(runtime, join(folder, "runtime"), { recursive: true });
  // `tar` on Windows 10+ writes zip archives.
  run("tar", ["-a", "-c", "-f", join(dist, "Forge-win-x64.zip"), "-C", dist, "Forge-win-x64"]);
  console.log(`Built ${folder}`);
}

if (process.platform === "darwin") buildMac();
else if (process.platform === "win32") buildWindows();
else {
  console.error("Packaging is for macOS and Windows.");
  process.exit(1);
}
