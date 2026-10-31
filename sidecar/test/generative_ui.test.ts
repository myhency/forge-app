import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  createGenerativeUiServer,
  generativeUiDefinitions,
  GENERATE_UI_TOOL,
  MAX_UI_CHARS,
  UI_GUIDE_TOOL,
  UI_SERVER_NAME,
} from "../src/generative_ui.js";
import { UI_EXAMPLES, UI_GUIDE_TOPICS, uiGuideText } from "../src/ui_guide.js";

const tool = () => generativeUiDefinitions().find((t) => t.name === "generate_sandboxed_ui")!;
const guide = () => generativeUiDefinitions().find((t) => t.name === "ui_guide")!;
async function run(args: Record<string, unknown>) {
  const done = await tool().handler(args as any, {});
  return { text: done.content.map((part: any) => part.text ?? "").join(""), isError: done.isError === true };
}

describe("generative ui tool", () => {
  it("names the server and the tool", () => {
    expect(UI_SERVER_NAME).toBe("forge_ui");
    expect(GENERATE_UI_TOOL).toBe("mcp__forge_ui__generate_sandboxed_ui");
    expect(UI_GUIDE_TOOL).toBe("mcp__forge_ui__ui_guide");
    expect(generativeUiDefinitions().map((t) => t.name).sort()).toEqual(["generate_sandboxed_ui", "ui_guide"]);
    expect(createGenerativeUiServer().type).toBe("sdk");
  });

  it("keeps both tools out of tool search so the model sees their descriptions", () => {
    for (const definition of generativeUiDefinitions()) expect(definition._meta?.["anthropic/alwaysLoad"]).toBe(true);
  });

  it("tells the model to call ui_guide first and to use the kit and the theme variables", () => {
    const text = tool().description;
    for (const word of ["ui_guide", "fx- classes", "--surface", "--c1", "vh units"]) expect(text).toContain(word);
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


describe("ui_guide", () => {
  const kitCss = readFileSync(new URL("../../crates/forge-entities/src/ui_kit.css", import.meta.url), "utf8");
  const cssClasses = new Set([...kitCss.matchAll(/\.(fx-[a-z0-9-]+)/g)].map((m) => m[1]));
  const usedClasses = (text: string) => new Set([...text.matchAll(/(?<![\w-])(fx-[a-z0-9-]+)/g)].map((m) => m[1]));
  const call = async (topic: string) => {
    const done = await guide().handler({ topic } as any, {});
    return { text: done.content.map((part: any) => part.text ?? "").join(""), isError: done.isError === true };
  };

  it("offers the kit and every reviewed example as a topic", () => {
    expect(UI_GUIDE_TOPICS[0]).toBe("kit");
    expect(UI_EXAMPLES.map((e) => e.name).sort()).toEqual(["calculator", "checklist_planner", "comparison_chart", "explainer_3d", "map_trip", "quiz", "tabbed_diagram"]);
  });

  it("the kit topic lists the classes and every example", async () => {
    const out = await call("kit");
    expect(out.isError).toBe(false);
    expect(out.text).toContain("fx-card");
    for (const example of UI_EXAMPLES) expect(out.text).toContain(example.name);
  });

  it("an example topic returns a complete call in the order the page streams", async () => {
    const out = await call("calculator");
    const order = ["initialHeight:", "placeholderMessages:", "html:", "jsFunctions:", "jsExpressions:"].map((k) => out.text.indexOf(k));
    expect(order.every((i) => i >= 0)).toBe(true);
    expect([...order].sort((a, b) => a - b)).toEqual(order);
    expect(uiGuideText("nope")).toBeNull();
  });

  it("every example page is valid: sized, kit classes that exist, no vh, no blocked hosts, images with onerror", () => {
    const hostOk = /^https:\/\/(cdnjs\.cloudflare\.com|cdn\.jsdelivr\.net|unpkg\.com|tiles\.openfreemap\.org|commons\.wikimedia\.org|upload\.wikimedia\.org|thumb\.wikimedia\.org)\//;
    for (const { name, page } of UI_EXAMPLES) {
      const all = [page.css, page.html, page.jsFunctions, page.jsExpressions].join("\n");
      expect(page.initialHeight, name).toBeGreaterThanOrEqual(100);
      expect(page.initialHeight, name).toBeLessThanOrEqual(800);
      expect(all.length, name).toBeLessThan(MAX_UI_CHARS / 3);
      expect(page.html.trim().length, name).toBeGreaterThan(0);
      const classes = usedClasses(all);
      expect(classes.size, `${name} uses the kit`).toBeGreaterThan(2);
      for (const c of classes) expect(cssClasses.has(c), `${name}: ${c} is not in the kit`).toBe(true);
      expect(/\b\d+vh\b/.test(all), `${name} sizes with vh`).toBe(false);
      for (const url of all.match(/https:\/\/[^"'\s)<>]+/g) ?? []) {
        if (url.includes("{z}") || url.startsWith("https://www.w3.org")) continue;
        expect(hostOk.test(url), `${name}: ${url}`).toBe(true);
      }
      for (const img of page.html.match(/<img\b[^>]*>/g) ?? []) expect(img, name).toContain("onerror");
    }
  });
});
