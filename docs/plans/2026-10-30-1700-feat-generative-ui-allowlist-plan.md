---
title: "feat: let interactive pages load libraries and map tiles from an allowlist"
type: feat
date: 2026-10-30
artifact_contract: ce-unified-plan/v1
product_contract_source: ce-brainstorm
execution: code
---

# feat: allowlisted libraries and map tiles for interactive pages

## Goal Capsule
- **Objective:** An interactive page Claude draws can look as good as the OpenIntelligentUI scenes: a real 3D model, a real map with tiles, charts from a library.
- **Means:** widen the frame's Content-Security-Policy from "nothing" to a short allowlist of script/style CDNs and map-tile hosts, keep connections closed, and tell Claude in the tool description which libraries to use (KTD1).
- **Execution profile:** one change on the open PR branch (`feat/generative-ui-card`).

## Product Contract
Settled (user-directed, 2026-10-30): open external access to an allowlist only; quality must be good. Rejected: opening all addresses (a page could send data out), bundling libraries into the app (map tiles still blocked), leaving it closed (pages stay crude).

- R1. A page can load scripts and styles from `cdnjs.cloudflare.com`, `cdn.jsdelivr.net` and `unpkg.com`.
- R2. A page can show images from those CDNs and from a fixed list of map-tile hosts (USGS, Esri, OpenStreetMap, CARTO) and `data:`/`blob:`.
- R3. A page still cannot `fetch`/XHR/WebSocket, submit a form, load a frame, or load an image from any other host: no way to send data out.
- R4. The tool description tells Claude to use three.js, Leaflet and similar libraries from the allowlist with pinned versions, and to build 3D from geometry (no model files) and maps with tile layers from the allowed hosts.
- R5. Harness samples show a real 3D airplane (three.js) and a real map (Leaflet with USGS tiles).

## Planning Contract
- KTD1: the policy is one const (`CSP` in `forge-entities/src/generated_ui.rs`); `connect-src` stays unset so `default-src 'none'` blocks connections. The only hosts in `img-src` are CDNs and tile servers, none of which an attacker controls, so an image request cannot carry data to them. A test pins that `connect-src`, `frame-src`, `form-action` and `default-src` stay closed and that every host is on the list.
- Risk: OpenStreetMap's tile servers may refuse requests without a referrer (the frame is an opaque origin); the samples use USGS, and the tool description names USGS and Esri first.

## Implementation Units
### U1. Policy and description
- Files: `crates/forge-entities/src/generated_ui.rs` (CSP + tests), `sidecar/src/generative_ui.ts` (description), `sidecar/test/generative_ui.test.ts`.
- Tests: CSP lists exactly the allowlist; `connect-src`/`frame-src` absent with `default-src 'none'`; description names the libraries and hosts.
### U2. Samples
- Files: `scripts/harness/add_readme_samples.py` (three.js plane, Leaflet map).
- Verification: real window, light mode; plane has lit solid model with axes; map shows USGS tiles with numbered pins and replay.

## Verification Contract
`cargo test -p forge-entities`, `cd sidecar && npx vitest run`, real window with `FORGE_HARNESS_EXTRA=scripts/harness/add_readme_samples.py`.

## Definition of Done
R1-R5 met and checked in the window; the PR body and spec state the new policy and the residual risk.
