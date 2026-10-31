---
title: "feat: maps from OpenFreeMap only (no usage limit, whole world)"
type: feat
date: 2026-10-30
artifact_contract: ce-unified-plan/v1
product_contract_source: ce-brainstorm
execution: code
---

# feat: OpenFreeMap as the only map source

## Goal Capsule
- **Objective:** A map Claude draws in an interactive page covers the whole world, shows labels in the local language, and does not depend on a tile service that limits use or needs a key.
- **Means:** MapLibre GL JS (from the library CDNs) drawing OpenFreeMap vector tiles; the frame's policy opens exactly one data host, `tiles.openfreemap.org`, and the other tile hosts are removed (KTD1).
- **Execution profile:** one change on the open PR branch.

## Product Contract
Settled (user-directed, 2026-10-30): "only maps without a usage limit; the whole world should show". Rejected: keeping OpenStreetMap raster tiles (usage policy limits heavy use), Esri (terms limit use), USGS (US only), CARTO (needs a key).
- R1. Pages load MapLibre GL JS and draw OpenFreeMap styles (liberty, bright, positron) anywhere in the world.
- R2. The only tile or data host the frame may use is `tiles.openfreemap.org`; `raster` hosts (OpenStreetMap, Esri, USGS, CARTO) are no longer allowed.
- R3. Everything else stays closed: no connection to any other host, no frames, forms or navigation.
- R4. The tool description tells Claude to use MapLibre with the OpenFreeMap style URL, with markers, popups, `flyTo`/`fitBounds`, and to make no satellite map (not offered).
- R5. Samples show the coastal trip and the Korea trip with MapLibre.

## Planning Contract
- KTD1: `connect-src https://tiles.openfreemap.org` (style, tiles, glyphs, sprites), `img-src` the same host plus `data:`/`blob:`, `worker-src blob:` and `child-src blob:` (MapLibre runs its tile workers from a blob). Residual risk, stated in the PR: a page can now send a request to that one host, so a script can put data into a URL there; the operator of that host could read it, an attacker could not. This is the first `connect-src` the frame has.
- KTD2: the policy lists the single data host in one const; a test pins `connect-src` to that host only and the absence of `frame-src`, `form-action` opening, and any other host.
- Risk: MapLibre needs WebGL (three.js already works in this web view).

## Implementation Units
- U1. Policy (`crates/forge-entities/src/generated_ui.rs`) and test.
- U2. Tool description (`sidecar/src/generative_ui.ts`) and test.
- U3. Samples (`scripts/harness/add_readme_samples.py`), then real-window check (map, Korea, hostile page).

## Verification Contract
`cargo test -p forge-entities`, `cd sidecar && npx vitest run`, real window: coastal and Korea maps draw with OpenFreeMap; hostile page (fetch, image/script from another host) still blocked.

## Definition of Done
R1-R5 met and checked in the window; PR body states the new connect-src and the removed hosts.
