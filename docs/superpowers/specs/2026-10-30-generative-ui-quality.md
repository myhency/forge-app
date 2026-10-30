# What it takes to reach OpenAI "Intelligent UI" quality

Written 2026-10-30. The OpenAI page (https://openai.com/ko-KR/index/gpt-6-for-everyone/) answered 403 to every automated fetch (bot protection) and was not opened; this note rests on press coverage of the same launch (TechCrunch 2026-10-07, Braindetox, Android Authority, ProPakistani) and on what Forge does today. Compare against screenshots of the real page before acting on the details.

## What the coverage says OpenAI does
- The model is **trained** to choose text, a visual or an interactive element per question, and to judge layout and interaction; training included an evaluation of the generated interface's clarity, usefulness and completeness.
- It composes **native, streamable components** (a shared design base) instead of free-form pages; the model decides the arrangement.
- A **streaming compiler** draws the interface while the model is still writing.
- Demo scenes: road-trip map, bill splitter, retirement calculator, bicycle diagram with tabs and selectable parts, roast-planning checklist with a "copy shopping list" button, Monty Hall, central limit theorem, GDP explained, a retro game. Simple questions still get text, and the person can ask for fewer visuals.

## Where Forge differs today
| Lever | OpenAI | Forge now |
|---|---|---|
| Look | one design system, components | Claude writes raw HTML/CSS each time: five colour variables, no shared components, so every page looks different and plain |
| Drawing | streams while writing | the card appears only when the whole call is in the transcript |
| Judgment | trained and evaluated | one tool description; Haiku ignored Forge's other tools |
| Quality loop | evaluated for clarity | none: a broken page is shown as is |
| Size | fits its content | fixed `initialHeight`, blank space or inner scroll |
| Media | curated | photos only by file name Claude must know |

## What to build, in order of effect per effort
1. **A Forge UI kit injected into every frame** (static CSS plus a few helpers in the srcdoc, no CDN): type scale, cards, tabs, segmented controls, sliders, stat tiles, tables, callouts, step lists, buttons, chart colours, both themes. Claude then writes semantic markup (`class="fx-card"`) and the result looks designed and consistent, with fewer tokens. This is the closest thing to OpenAI's component library.
2. **Auto height**: the frame reports its content height (ResizeObserver, same bridge, nonce-checked) and the card follows it up to a cap.
3. **Gold examples as a Forge skill** (`skills_plugin.ts` already loads skills): 8 to 10 reviewed pages (calculator, comparison with chart, map trip, 3D explainer, tabbed diagram, checklist with copy, quiz that sends its result back) plus a design checklist and a "when not to use a page" rule. Few-shot examples move quality more than prose rules.
4. **An evaluation loop with the harness**: 20 fixed prompts, render each in the real window, capture screenshots, score with a rubric (clarity, polish, errors), and compare descriptions or models (Opus, Sonnet, Haiku) before changing anything. Without it every tweak is a guess.
5. **Page errors back to Claude**: capture `window.onerror` in the frame, show "the page failed" with a button that sends the error as the next message so Claude fixes it. Cheap and removes the worst failures.
6. **Streaming draw**: forward partial tool input (`includePartialMessages` is already on), parse partial JSON in order, and update the frame progressively. Most visible polish, and the most work.
7. **A sidecar image tool** (`find_image(query)` using the Wikimedia API from the sidecar, which is not bound by the frame's policy) returning verified URLs, size, author and licence, so Claude never invents file names and can credit properly.
8. **A "visuals" setting** (auto, fewer, always) like OpenAI's, replacing the on/off switch.

## Risks and limits
- Prompting alone caps out; OpenAI's quality comes partly from training and evaluation we cannot copy, so the kit plus examples plus an eval loop is the realistic substitute.
- The kit adds a stable style surface that must be kept in sync with Forge's themes.
- Each item that opens a new capability (image tool, streaming) widens what a page can trigger; keep the frame policy and the nonce-checked bridge as the only ways out.
