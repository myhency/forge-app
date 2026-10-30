# Interactive page card (generate_sandboxed_ui)

Plan: `docs/plans/2026-10-30-1500-feat-generative-ui-card-plan.md`. Pattern source: CopilotKit/OpenIntelligentUI's `generateSandboxedUi`.

## What it does
Claude can call `mcp__forge_ui__generate_sandboxed_ui` (`initialHeight`, `placeholderMessages`, `css`, `html`, `jsFunctions`, `jsExpressions`). The call becomes a card in the chat (`Item::GeneratedUi`): an `<iframe sandbox="allow-scripts" srcdoc>` with a Content-Security-Policy of `default-src 'none'` plus an allowlist: scripts, styles, fonts and images from `cdnjs.cloudflare.com`, `cdn.jsdelivr.net` and `unpkg.com` (three.js, MapLibre, chart libraries), and one data host, `tiles.openfreemap.org` (`connect-src` and `img-src`), which serves map styles, vector tiles, fonts and sprites for the whole world with no key and no usage limit. `worker-src blob:` is what MapLibre's tile workers run from. Every other connection, frame, form and host stays closed. Residual risk: a page can now put data into a request URL to that one host (its operator could read it, an attacker could not); this is the only `connect-src`. The tool description tells Claude which libraries and tile URLs to use. Setting: Settings > Code > `Interactive pages` (`AppConfig.generative_ui`, default on).

## Isolation (checked in the real window, 2026-10-30)
- `fetch`, `window.parent.document`, `localStorage`, an image or script from `example.com`: blocked. Map tiles, styles and fonts from OpenFreeMap load. Images: inline SVG, `data:`, canvas and files from the three library CDNs show; Wikimedia and Unsplash photos are blocked. Checked again after the allowlist was added.
- IPC: wry reports the sending frame's URL, and for a srcdoc frame it is `about:srcdoc`. The vendored `dioxus-desktop` (`vendor/dioxus-desktop`, `[patch.crates-io]`) drops every IPC message whose URL is not `dioxus://`, `http(s)://dioxus.`, and logs `forge: dropped an IPC message from about:srcdoc`. Forge's own clicks still work. On Windows wry reports the message's `Source` (the top-level document, not clearly the frame), so the check may not tell a frame from the app: the setting is therefore off by default on Windows until someone proves it with `add_generated_ui_samples.py`.
- Links: the page's bridge script cancels every click on an `<a>`.
- Known gap: a page can still set `location.href` to an http(s) address, which Dioxus' navigation handler opens in the system browser (same as the html file preview).

## Bridge
`forge.send({label, values})` posts `{n: nonce, label, values}` to the parent. The parent listener accepts it only from a frame of ours that holds the focus (the person clicked in it), then `bridge_message` checks the nonce, at most 20 values of text, number or flag, no leading `/` or `!`, and builds Forge's own sentence (`Label — key: value, ...`), sent as the person's message through the same `on_send` the end card uses. One message per second per card. A page that knows its nonce (it can read its own source) can post once it has the focus without pressing its button; the worst it can send is such a sentence, visible in the chat as the person's message.

## Left
- Streaming draw: the transcript carries a call only when complete, so `placeholderMessages` rarely show.
- Frame height is fixed at `initialHeight` (100-800); the page scrolls inside.
- Windows, Haiku compliance and a real SDK turn were not run (see the PR).
