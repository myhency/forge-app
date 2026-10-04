// Bundles src/main.js into the one file Forge puts in the window head.
import { build } from "esbuild";
import { writeFileSync } from "node:fs";

const out = new URL("../../crates/forge-shared/assets/editor.js", import.meta.url);
const result = await build({
  entryPoints: ["src/main.js"],
  bundle: true,
  minify: true,
  format: "iife",
  target: "safari15",
  legalComments: "none",
  write: false,
});
// The code goes inline between <script> tags: a literal `</script` inside it would end the script.
const text = result.outputFiles[0].text.replaceAll("</script", "<\\/script");
writeFileSync(out, text);
console.log(`wrote ${out.pathname} (${(text.length / 1024).toFixed(0)} KiB)`);
