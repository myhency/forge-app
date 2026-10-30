import { describe, expect, it } from "vitest";
import {
  createGenerativeUiServer,
  generativeUiDefinitions,
  GENERATE_UI_TOOL,
  MAX_UI_CHARS,
  UI_SERVER_NAME,
} from "../src/generative_ui.js";

const tool = () => generativeUiDefinitions()[0]!;
async function run(args: Record<string, unknown>) {
  const done = await tool().handler(args as any, {});
  return { text: done.content.map((part: any) => part.text ?? "").join(""), isError: done.isError === true };
}

describe("generative ui tool", () => {
  it("names the server and the tool", () => {
    expect(UI_SERVER_NAME).toBe("forge_ui");
    expect(GENERATE_UI_TOOL).toBe("mcp__forge_ui__generate_sandboxed_ui");
    expect(generativeUiDefinitions().map((t) => t.name)).toEqual(["generate_sandboxed_ui"]);
    expect(createGenerativeUiServer().type).toBe("sdk");
  });

  it("is kept out of tool search so the model sees its description", () => {
    expect(tool()._meta?.["anthropic/alwaysLoad"]).toBe(true);
  });

  it("names the allowlist and the libraries the page may use, and asks for quality", () => {
    const text = tool().description;
    for (const word of ["cdnjs.cloudflare.com", "cdn.jsdelivr.net", "unpkg.com", "tiles.openfreemap.org", "maplibre-gl", "three.js", "MapLibre"]) {
      expect(text).toContain(word);
    }
    expect(text).toContain("polished product");
    expect(text).not.toContain("no network");
    expect(text).toContain("including Korea");
    expect(text).toContain("maplibregl.Map");
    expect(text).toContain("Special:FilePath");
    expect(text).toContain("onerror");
    expect(text).not.toContain("leaflet");
    expect(text).not.toContain("tile.openstreetmap.org/{z}");
  });

  it("lists the parameters in the order the page streams", () => {
    expect(Object.keys((tool().inputSchema as any).shape ?? tool().inputSchema)).toEqual([
      "initialHeight",
      "placeholderMessages",
      "css",
      "html",
      "jsFunctions",
      "jsExpressions",
    ]);
  });

  it("accepts a page and tells the model to describe it in a sentence or two", async () => {
    const out = await run({ html: "<p>hi</p>" });
    expect(out.isError).toBe(false);
    expect(out.text).toContain("one or two sentences");
  });

  it("refuses an empty page and an oversized one with a readable text", async () => {
    expect(await run({ html: "  " })).toMatchObject({ isError: true });
    const big = await run({ html: "x".repeat(MAX_UI_CHARS + 1) });
    expect(big.isError).toBe(true);
    expect(big.text).toContain("too large");
  });
});
