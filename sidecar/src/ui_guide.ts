import calculator from "./ui_examples/calculator.json" with { type: "json" };
import checklistPlanner from "./ui_examples/checklist_planner.json" with { type: "json" };
import comparisonChart from "./ui_examples/comparison_chart.json" with { type: "json" };
import explainer3d from "./ui_examples/explainer_3d.json" with { type: "json" };
import kit from "./ui_examples/kit.json" with { type: "json" };
import mapTrip from "./ui_examples/map_trip.json" with { type: "json" };
import quiz from "./ui_examples/quiz.json" with { type: "json" };
import tabbedDiagram from "./ui_examples/tabbed_diagram.json" with { type: "json" };

export interface UiPage {
  initialHeight: number;
  placeholderMessages: string[];
  css: string;
  html: string;
  jsFunctions: string;
  jsExpressions: string;
}
export interface UiExample {
  name: string;
  title: string;
  use_for: string;
  page: UiPage;
}

/** The reviewed pages, written by scripts/make_ui_examples.py. They are the bar Claude is shown and the baseline
 *  the evaluation measures against. */
export const UI_EXAMPLES: UiExample[] = [calculator, comparisonChart, mapTrip, explainer3d, tabbedDiagram, checklistPlanner, quiz];

export const UI_KIT_MARKDOWN: string = kit.markdown;

export const UI_GUIDE_TOPICS = ["kit", ...UI_EXAMPLES.map((example) => example.name)] as [string, ...string[]];

function fence(label: string, language: string, body: string): string {
  return body.trim() ? `${label}\n\`\`\`${language}\n${body}\n\`\`\`` : "";
}

/** What `ui_guide` answers: the kit reference with the list of examples, or one complete example call to adapt. */
export function uiGuideText(topic: string): string | null {
  if (topic === "kit") {
    const list = UI_EXAMPLES.map((example) => `- ${example.name}: ${example.use_for}`).join("\n");
    return `${UI_KIT_MARKDOWN}\n\n## Reviewed examples (call ui_guide with the name)\n${list}`;
  }
  const example = UI_EXAMPLES.find((candidate) => candidate.name === topic);
  if (!example) return null;
  const { page } = example;
  return [
    `# Example: ${example.title}`,
    `Use it for: ${example.use_for}`,
    "This is a complete, reviewed call of generate_sandboxed_ui. Keep its structure, the kit classes and the level of polish; change the subject, the content and the numbers. Send the parameters in the order shown.",
    `initialHeight: ${page.initialHeight}`,
    `placeholderMessages: ${JSON.stringify(page.placeholderMessages)}`,
    fence("css:", "css", page.css),
    fence("html:", "html", page.html),
    fence("jsFunctions:", "js", page.jsFunctions),
    fence("jsExpressions:", "js", page.jsExpressions),
  ]
    .filter(Boolean)
    .join("\n\n");
}
