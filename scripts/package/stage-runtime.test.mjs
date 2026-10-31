import assert from "node:assert/strict";
import test from "node:test";

import { expectedSha, nodeArchive, parseArgs } from "./stage-runtime.mjs";

test("the Node archive of each target and the program inside it", () => {
  assert.deepEqual(nodeArchive("22.22.1", "darwin", "arm64"), {
    file: "node-v22.22.1-darwin-arm64.tar.gz",
    program: "node-v22.22.1-darwin-arm64/bin/node",
    name: "node",
  });
  assert.deepEqual(nodeArchive("22.22.1", "win32", "x64"), {
    file: "node-v22.22.1-win-x64.zip",
    program: "node-v22.22.1-win-x64/node.exe",
    name: "node.exe",
  });
});

test("an unknown platform or arch is refused", () => {
  assert.throws(() => nodeArchive("22.22.1", "linux", "x64"), /Unsupported platform/);
  assert.throws(() => nodeArchive("22.22.1", "win32", "ia32"), /Unsupported arch/);
});

test("the expected sha is read from SHASUMS256.txt by exact file name", () => {
  const shasums = "aaa111  node-v22.22.1-win-x64.zip\nbbb222  node-v22.22.1-win-x64.zip.sig\nccc333  node-v22.22.1-darwin-arm64.tar.gz\n";
  assert.equal(expectedSha(shasums, "node-v22.22.1-win-x64.zip"), "aaa111");
  assert.equal(expectedSha(shasums, "node-v22.22.1-darwin-arm64.tar.gz"), "ccc333");
  assert.throws(() => expectedSha(shasums, "node-v1-win-x64.zip"), /not in SHASUMS256/);
});

test("options are read as name/value pairs over the defaults", () => {
  assert.deepEqual(parseArgs(["--arch", "x64"], { arch: "arm64", out: "o" }), { arch: "x64", out: "o" });
  assert.throws(() => parseArgs(["--arch"], {}), /Expected/);
});
