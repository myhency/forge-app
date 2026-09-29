import { PassThrough } from "node:stream";
import { describe, expect, it } from "vitest";
import { readCommands } from "../src/io.js";

describe("readCommands", () => {
  it("reports when the input ends, so an orphaned sidecar can exit", async () => {
    const input = new PassThrough();
    const seen: string[] = [];
    const closed = new Promise<void>((resolve) => {
      readCommands(
        input,
        (command) => seen.push(command.type),
        () => {},
        () => resolve(),
      );
    });

    input.write('{"type":"interrupt","id":"1"}\n');
    input.end();
    await closed;

    expect(seen).toEqual(["interrupt"]);
  });
});
