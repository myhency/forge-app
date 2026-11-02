import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import { installDir, integrityMatches } from "../src/install_agent_desktop.js";

describe("install_agent_desktop", () => {
  it("accepts a package only when its sha512 matches the registry's integrity string", () => {
    const data = new TextEncoder().encode("package bytes");
    const good = `sha512-${createHash("sha512").update(data).digest("base64")}`;
    expect(integrityMatches(data, good)).toBe(true);
    expect(integrityMatches(new TextEncoder().encode("other bytes"), good)).toBe(false);
    expect(integrityMatches(data, "sha1-abc")).toBe(false);
    expect(integrityMatches(data, "")).toBe(false);
  });

  it("keeps the program in an agent-desktop folder of the tools folder", () => {
    expect(installDir("/t")).toBe("/t/agent-desktop");
  });
});
