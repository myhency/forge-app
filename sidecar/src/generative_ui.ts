import { createSdkMcpServer, tool } from "@anthropic-ai/claude-agent-sdk";
import type { McpSdkServerConfigWithInstance, SdkMcpToolDefinition } from "@anthropic-ai/claude-agent-sdk";
import { z } from "zod";
import { UI_GUIDE_TOPICS, uiGuideText } from "./ui_guide.js";

export const UI_SERVER_NAME = "forge_ui";
export const GENERATE_UI_TOOL = `mcp__${UI_SERVER_NAME}__generate_sandboxed_ui`;
export const UI_GUIDE_TOOL = `mcp__${UI_SERVER_NAME}__ui_guide`;

/** The page is kept in the transcript and in the chat, so its size is capped here and again by the app. */
export const MAX_UI_CHARS = 200_000;
const MAX_PLACEHOLDERS = 6;

const DESCRIPTION = [
  "Before your first page in a conversation, call ui_guide (topic 'kit', or the name of the example closest to what you will draw) and build on what it returns: the page starts with the Forge UI kit loaded, so write semantic markup with its fx- classes instead of your own CSS, and follow the reviewed example. Pages that ignore the kit look unfinished.",
  "Draw a small interactive page in the chat: a calculator, a chart, a comparison with controls, a 3D or step-by-step explanation, a planner. Use it only when a page the person can change or explore explains more than text or a plain table would. For a plain answer, a list, or code, write text.",
  "The page runs in an isolated frame that can load only an allowlist: scripts and styles from cdnjs.cloudflare.com, cdn.jsdelivr.net and unpkg.com, map data from tiles.openfreemap.org (OpenFreeMap: styles, vector tiles, fonts, sprites for the whole world, free, no key, no usage limit), and photos from upload.wikimedia.org, thumb.wikimedia.org and commons.wikimedia.org (Wikimedia Commons, images only). It cannot fetch data from any other address, call an API, submit a form, open a frame or load any other image or font, and it has no storage and no access to the app. Anything else you need (data, geometry, text) goes inside the page.",
  "Aim for the quality of a polished product, not a sketch. Use a library when the subject needs one, loading it with a <script src> tag at the top of html with a pinned version: three.js (https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js) for 3D, MapLibre GL JS (https://cdnjs.cloudflare.com/ajax/libs/maplibre-gl/4.7.1/maplibre-gl.js and maplibre-gl.css) for maps, Chart.js or D3 for charts. For 3D, build the model from geometry with lights, materials, labeled axes and drag-to-rotate; do not load model files. For every map, anywhere in the world including Korea, use MapLibre GL JS with an OpenFreeMap style: new maplibregl.Map({container, style: 'https://tiles.openfreemap.org/styles/liberty', center: [longitude, latitude], zoom}) (longitude first). Other styles: .../styles/bright and .../styles/positron (light, minimal). Labels come in the local language, so Korean places show in Hangul. Give the container an explicit pixel height, add maplibregl.NavigationControl, numbered markers as maplibregl.Marker with your own element, popups, flyTo or fitBounds, and a line with a geojson source and a line layer after the map's 'load' event. Do not use any other tile provider or satellite imagery (OpenStreetMap, Esri, USGS, CARTO, Mapbox, Google, Naver and Kakao are blocked, need a key, or limit use). For photos use only Wikimedia Commons, through https://commons.wikimedia.org/wiki/Special:FilePath/<exact file name>?width=640 (a file name you know exists, or one you found with a search tool; never invent one). The page cannot look files up, so every <img> must have an onerror handler that hides it or shows a plain placeholder, and the photo's credit (a 'Photo: Wikimedia Commons' line, or the author if known) must appear next to it. Icons and illustrations are better drawn as inline SVG. Do not use photos or icons from any other site.",
  "Send the parameters in this order, because the page starts drawing while they stream: initialHeight, placeholderMessages, css, html, jsFunctions, jsExpressions.",
  "initialHeight: height in pixels while it loads (100-800). placeholderMessages: 1-6 short lines shown while the page is written. css: the styles. html: the body markup (no <html> or <head>; <script src> and <link> tags for allowlisted libraries go first). jsFunctions: the script's function declarations. jsExpressions: statements run once after the page is drawn, to set it up.",
  "Use the theme variables --fg, --bg, --muted, --line, --surface, --accent and the chart colours --c1 to --c6 so the page follows the app's light and dark theme; never hard-code white, black or grey. The card follows the page's height by itself: never size anything with vh units or height:100%. Make controls work on their own inside the page.",
  "To let the person send a result back to you, call forge.send({ label: 'short name', values: { key: value } }) from a click handler; it shows in the chat as the person's next message (at most 20 values of text, number or true/false). Call it only from a button the person presses.",
  "Each call draws a new page; you cannot edit an earlier one. After the call, write one or two sentences about what the page shows; do not repeat its content.",
].join("\n\n");

const ALWAYS_LOADED = { alwaysLoad: true };

function answer(text: string, isError = false) {
  return { content: [{ type: "text" as const, text }], ...(isError ? { isError: true } : {}) };
}

export function generativeUiDefinitions(): SdkMcpToolDefinition<any>[] {
  const generate = tool(
    "generate_sandboxed_ui",
    DESCRIPTION,
    {
      initialHeight: z.number().optional(),
      placeholderMessages: z.array(z.string()).max(MAX_PLACEHOLDERS).optional(),
      css: z.string().optional(),
      html: z.string(),
      jsFunctions: z.string().optional(),
      jsExpressions: z.string().optional(),
    },
    async (args) => {
      if (!args.html.trim()) {
        return answer("The page has no html. Send the markup in html.", true);
      }
      const size = [args.css, args.html, args.jsFunctions, args.jsExpressions].reduce((n, part) => n + (part?.length ?? 0), 0);
      if (size > MAX_UI_CHARS) {
        return answer(`The page is too large (${size} characters; the limit is ${MAX_UI_CHARS}). Send a smaller one.`, true);
      }
      return answer("Shown in the chat as an interactive page. Now write one or two sentences about what it shows; do not repeat its content.");
    },
    ALWAYS_LOADED,
  );
  const guide = tool(
    "ui_guide",
    "Returns the Forge UI kit (the fx- classes every page starts with), the design rules, and one reviewed, complete example call of generate_sandboxed_ui to adapt. Call it once before your first interactive page; topic 'kit' lists the examples.",
    { topic: z.enum(UI_GUIDE_TOPICS) },
    async (args) => {
      const text = uiGuideText(args.topic);
      return text ? answer(text) : answer(`Unknown topic ${args.topic}.`, true);
    },
    ALWAYS_LOADED,
  );
  return [guide, generate];
}

export function createGenerativeUiServer(): McpSdkServerConfigWithInstance {
  return createSdkMcpServer({ name: UI_SERVER_NAME, tools: generativeUiDefinitions(), alwaysLoad: true });
}
