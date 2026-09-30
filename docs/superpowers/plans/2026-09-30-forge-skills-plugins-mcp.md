# Forge Skills, Plugins and MCP Servers Implementation Plan (Plan 7)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the `enabled_skills` / `enabled_plugins` / `enabled_mcp_servers` lists real: three settings tabs manage them, and every session started afterwards runs with exactly the skills, plugins and MCP servers that are switched on — and with nothing inherited from `~/.claude/settings.json`.

**Architecture:** `forge-domain` gets `ActiveExtensions` (what a session starts with) and a pure `select` that filters definitions by the config's enabled lists. `forge-application` gets `ExtensionsUseCase` (resolve at session start, list definitions for the tabs, toggle). The live-session actor resolves the set right before it spawns a sidecar and puts it in `StartSessionParams`; the wire protocol carries it as `skills` / `plugins` / `mcp_servers`; the Node sidecar maps it to SDK options with `settingSources: ['project']` and `strictMcpConfig`. Because the spike (spec §17.7) showed that user skills are not discovered under `['project']` and that `['user','project']` leaks the user's plugins, enabled skills are delivered through a per-session generated local plugin `forge-skills` (symlinks to each enabled skill's directory) that the sidecar builds and removes. The settings overlay gets Skills / Plugins / MCP servers tabs over pure row builders in `forge-entities`.

**Tech Stack:** Rust 2021, Dioxus 0.7.10 desktop, tokio, mockall; Node 22.22.1, TypeScript, vitest, `@anthropic-ai/claude-agent-sdk` 0.3.283.

**Spec:** `docs/superpowers/specs/2026-09-27-forge-design.md` §17 (§4.3 for the activation split, §13–14 for the definition readers, §16.9 for the two carried-over items).

## Global Constraints

- Hexagonal crate graph is enforced by `Cargo.toml` dependencies: `forge-domain` depends on nothing of ours; `forge-application` on domain + ports only; `forge-features` / `forge-widgets` / `forge-pages` must not name a concrete adapter. Only `forge-app/src/main.rs` names adapters.
- Activation state is Forge's own: `enabled_*` lists in Forge's `config.json`, by name, global (spec §4.1, §4.3). Forge never reads or writes Claude Code's `enabledPlugins`.
- A Forge session loads only the project settings source and only what Forge has enabled: `settingSources: ['project']`, `strictMcpConfig: true` (spec §17.1). Consequence to keep in the UI copy and docs: user-level hooks and permission rules from `~/.claude/settings.json` no longer apply.
- Skills reach the SDK only through the generated `forge-skills` plugin, listed as `forge-skills:<name>` (spec §17.3, §17.7). A skill name that is not one safe path component (empty, `.`, `..`, or containing `/`, `\` or NUL) is skipped. The temporary plugin directory is removed when the session ends and removing it must never touch the skill directories it links to.
- An empty active list is sent as `[]` / `{}`, never omitted — the SDK treats an omitted `skills` as "CLI defaults", not "off" (spec §17.2).
- Toggles apply to sessions started afterwards; a running session keeps what it started with. Every tab says: "Sessions that are already open keep what they started with; a change applies to the next new session." (spec §17.4).
- A reader failure while something is enabled fails the session start with the existing notice mechanism — a session never starts with fewer tools than switched on (spec §17.3).
- Saves use `dioxus::core::spawn_forever` so an unmount (tab switch, Esc) cannot cancel them half-way (spec §15.11, §16.9).
- Marketplace install, skill editing and MCP add/remove are out of scope (spec §2, §17.1). Do not touch the other Minor items of §16.9.
- Pinned versions: `@anthropic-ai/claude-agent-sdk` 0.3.283, Node 22.22.1, Dioxus `=0.7.10`.
- UI text is English (the existing UI is English).
- Every commit message ends with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Test commands: `cargo test -p <crate>`; sidecar `cd sidecar && npm test`; full check `cargo test --workspace` plus `cd sidecar && npm test && npm run typecheck` (baseline before this plan: 274 Rust tests; the `block v0.1.6` future-incompat notice is expected).

## Review Focus

Inputs the spec implies but that are easy to get wrong, most likely first. Each has a test in the task named.

1. An enabled name with no definition any more (a deleted skill, a project-local skill while another project is open) → skipped silently, the session still starts (Task 2).
2. Nothing enabled → the sidecar still receives `skills: []`, `plugins: []`, `mcp_servers: {}` and passes empty values to the SDK, so Claude Code's own defaults do not come back (Tasks 3 and 4).
3. A definition reader fails while an extension is enabled → the session does not start, no process is spawned, the typed text comes back to the box (Task 6). With nothing enabled, a broken reader must not block sessions (Task 5).
4. Two definitions with the same name (a user skill and a project skill; a plugin installed under two scopes) → listed once in the tab, sent once to the SDK (Tasks 2 and 7).
5. A skill whose name is not a safe path component (for example `../evil`) → skipped by the sidecar, nothing is created outside the temp plugin directory; and ending a session deletes the temp plugin directory but not the real skill directories behind its symlinks (Task 4).
6. An MCP server with neither `command` nor `url` → its row is still shown, with an empty detail, and can be toggled (Task 7).

---

## File Structure

| File | Change |
|---|---|
| `crates/forge-domain/src/extension.rs` | `PluginDefinition::key`, `ActiveExtensions`, `ActiveExtensions::select` |
| `crates/forge-domain/src/lib.rs` | export `ActiveExtensions` |
| `crates/forge-ports/src/sidecar_port.rs` | `StartSessionParams.extensions` |
| `crates/forge-adapter-sidecar/src/protocol.rs` | wire fields `skills` / `plugins` / `mcp_servers` |
| `crates/forge-adapter-sidecar/tests/fixtures/fake_sidecar.js` | remembers the start command, echoes its extension fields |
| `crates/forge-adapter-sidecar/tests/contract_test.rs` | extension round-trip test |
| `sidecar/src/protocol.ts`, `sidecar/src/session.ts` | wire type, SDK option mapping |
| `sidecar/src/skills_plugin.ts` | **new** builds/removes the per-session `forge-skills` plugin directory |
| `sidecar/test/session.test.ts` | option mapping tests |
| `crates/forge-application/src/use_cases/extensions.rs` | **new** `ExtensionsUseCase`, `ExtensionsError` |
| `crates/forge-application/src/{live_session,live_session_actor,registry,test_support}.rs` | resolve at sidecar start |
| `crates/forge-entities/src/extension_rows.rs` | **new** `ExtensionRow`, row builders, `mcp_summary`, `project_label` |
| `crates/forge-shared/src/components.rs`, `theme.css` | `Switch` component and its CSS |
| `crates/forge-features/src/{shortcut,workspace,settings,section_status}.rs` | shortcut gating, tagged status, tabs state |
| `crates/forge-widgets/src/settings_overlay.rs` | three new tabs |
| `crates/forge-pages/src/workspace_page.rs` | overlay-aware keys, project sync |
| `crates/forge-app/src/main.rs` | wire the readers into `ExtensionsUseCase` |
| `docs/superpowers/specs/2026-09-27-forge-design.md` | §17.7 spike findings, §17.8 as built |

---

### Task 1: Spike — what does the real SDK do with these options? (throwaway)

**Files:**
- Create (do **not** commit; delete at the end of the task): `sidecar/spike-extensions.mjs`
- Modify: `docs/superpowers/specs/2026-09-27-forge-design.md` (§17.7 only)

**Interfaces:**
- Consumes: the real `@anthropic-ai/claude-agent-sdk` 0.3.283 and the real `~/.claude`.
- Produces: §17.7 findings that Tasks 3–4 rely on: (a) whether a user skill is discovered under `settingSources: ['project']` (it is not — see §17.7, which drives the generated-plugin design of Tasks 2–4); (b) a plugin loads from its `installPath`; (c) both MCP config shapes are accepted; (d) nothing from `~/.claude/settings.json` leaks in. It also settles the `settingSources` value used in Task 4.

- [ ] **Step 1: Install the sidecar's dependencies**

Run: `cd sidecar && npm ci`
Expected: installs without error (`node_modules/@anthropic-ai/claude-agent-sdk` exists).

- [ ] **Step 2: Write the throwaway script**

Create `sidecar/spike-extensions.mjs`:

```js
// THROWAWAY (Plan 7, Task 1). Prints what the SDK reports as loaded for one option set.
import { query } from "@anthropic-ai/claude-agent-sdk";
import { homedir } from "node:os";
import { readFileSync } from "node:fs";

const caseName = process.argv[2];
const skills = JSON.parse(process.env.SPIKE_SKILLS ?? "[]");
const installed = JSON.parse(readFileSync(`${homedir()}/.claude/plugins/installed_plugins.json`, "utf8"));
const firstKey = Object.keys(installed.plugins)[0];
const pluginPath = installed.plugins[firstKey][0].installPath;

const cases = {
  baseline: {},
  isolated: { settingSources: ["project"], strictMcpConfig: true, skills: [], plugins: [], mcpServers: {} },
  enabled: {
    settingSources: ["project"],
    strictMcpConfig: true,
    skills,
    plugins: [{ type: "local", path: pluginPath }],
    mcpServers: {
      stdio_shape: { command: "node", args: ["-e", "setInterval(() => {}, 1000)"] },
      http_shape: { type: "http", url: "http://127.0.0.1:9/mcp" },
    },
  },
  userSources: { settingSources: ["user", "project"], strictMcpConfig: true, skills, plugins: [], mcpServers: {} },
};
const options = cases[caseName];
if (!options) throw new Error(`unknown case ${caseName}`);

async function* prompt() {
  yield { type: "user", message: { role: "user", content: "Reply with the single word ok." }, parent_tool_use_id: null };
  await new Promise(() => {});
}

setTimeout(() => { console.error("timeout waiting for init"); process.exit(2); }, 90000);
const q = query({ prompt: prompt(), options: { cwd: process.cwd(), permissionMode: "default", ...options } });
for await (const m of q) {
  if (m.type === "system" && m.subtype === "init") {
    console.log(JSON.stringify({ case: caseName, skills: m.skills, plugins: m.plugins?.map((p) => p.name), mcp_servers: m.mcp_servers }, null, 2));
    process.exit(0);
  }
}
```

- [ ] **Step 3: Run the four cases**

Pick a real user skill: `ls ~/.claude/skills | head` (use one directory name whose `SKILL.md` has a `name:`). If `init` is not reached without credentials, first `export ANTHROPIC_API_KEY=…` from Forge's config (`~/Library/Application Support/Forge/config.json` on macOS; never paste the key into a file or a commit).

Run, from `sidecar/`:

```bash
export SPIKE_SKILLS='["<skill-name>"]'
for c in baseline isolated enabled userSources; do node spike-extensions.mjs $c; done
```

Expected: four JSON blocks. Read them against these questions:
- `isolated`: `skills`, `plugins`, `mcp_servers` should all be empty. If not, the isolation options do not isolate → **stop and ask the human**; spec §17.1 must be revisited.
- `enabled`: (a) `<skill-name>` is in `skills`; (b) the plugin's name is in `plugins`; (c) both `stdio_shape` and `http_shape` are in `mcp_servers` (a `failed` status is fine — the fake servers are not real; being *accepted* is the point).
- `baseline`: shows what leaks today (d). Compare with `isolated`.
- `userSources`: only matters if (a) failed. If `<skill-name>` appears here but `plugins` and `mcp_servers` stay empty, Task 4 uses `settingSources: ['user', 'project']` and §17.7 records that user-level hooks and rules apply again. If plugins or MCP servers leak here, **stop and ask the human**.

- [ ] **Step 4: Record the findings and clean up**

Replace the `*(filled in by Task 1)*` line of spec §17.7 with: the date, the SDK version, a four-row table (case, skills, plugins, mcp_servers as observed), the answers to (a)–(d), and the `settingSources` value Task 4 will use. Then:

```bash
rm sidecar/spike-extensions.mjs
git status --short   # only the spec file is modified
git add docs/superpowers/specs/2026-09-27-forge-design.md
git commit -m "docs: record the Plan 7 spike findings

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Domain — `ActiveExtensions` and `select`

**Files:**
- Modify: `crates/forge-domain/src/extension.rs`
- Modify: `crates/forge-domain/src/lib.rs`

**Interfaces:**
- Consumes: `AppConfig` (`enabled_skills`, `enabled_plugins`, `enabled_mcp_servers`), `SkillDefinition`, `PluginDefinition`, `McpServerDefinition`.
- Produces:
  - `PluginDefinition::key(&self) -> String` — `name@marketplace`, or `name` when there is no marketplace (the string stored in `enabled_plugins`).
  - `#[derive(Debug, Clone, PartialEq, Default)] pub struct ActiveExtensions { pub skills: BTreeMap<String, PathBuf>, pub plugin_paths: Vec<PathBuf>, pub mcp_servers: BTreeMap<String, serde_json::Value> }` — `skills` maps an enabled skill's name to its directory
  - `ActiveExtensions::select(config: &AppConfig, skills: &[SkillDefinition], plugins: &[PluginDefinition], servers: &[McpServerDefinition]) -> ActiveExtensions`

- [ ] **Step 1: Write the failing tests**

In `crates/forge-domain/src/extension.rs`, add to the `tests` module:

```rust
    fn skill(name: &str) -> SkillDefinition {
        SkillDefinition { name: name.to_string(), path: PathBuf::from(format!("/skills/{name}")), description: None }
    }

    fn plugin(name: &str, marketplace: Option<&str>, path: &str) -> PluginDefinition {
        PluginDefinition {
            name: name.to_string(),
            marketplace: marketplace.map(str::to_string),
            path: PathBuf::from(path),
        }
    }

    fn server(name: &str) -> McpServerDefinition {
        McpServerDefinition { name: name.to_string(), config: serde_json::json!({"command": name}) }
    }

    #[test]
    fn a_plugin_key_is_name_at_marketplace_or_just_the_name() {
        assert_eq!(plugin("hud", Some("official"), "/p").key(), "hud@official");
        assert_eq!(plugin("hud", None, "/p").key(), "hud");
    }

    #[test]
    fn select_keeps_only_what_is_enabled() {
        let config = AppConfig {
            enabled_skills: vec!["pdf".to_string()],
            enabled_plugins: vec!["hud@official".to_string()],
            enabled_mcp_servers: vec!["files".to_string()],
            ..AppConfig::default()
        };
        let active = ActiveExtensions::select(
            &config,
            &[skill("pdf"), skill("docx")],
            &[plugin("hud", Some("official"), "/plugins/hud"), plugin("other", Some("official"), "/plugins/other")],
            &[server("files"), server("browser")],
        );
        assert_eq!(active.skills.keys().collect::<Vec<_>>(), vec!["pdf"]);
        assert_eq!(active.skills["pdf"], PathBuf::from("/skills/pdf"));
        assert_eq!(active.plugin_paths, vec![PathBuf::from("/plugins/hud")]);
        assert_eq!(active.mcp_servers.keys().collect::<Vec<_>>(), vec!["files"]);
        assert_eq!(active.mcp_servers["files"]["command"], "files");
    }

    #[test]
    fn select_skips_an_enabled_name_that_has_no_definition() {
        let config = AppConfig {
            enabled_skills: vec!["deleted".to_string(), "pdf".to_string()],
            enabled_plugins: vec!["gone@nowhere".to_string()],
            enabled_mcp_servers: vec!["missing".to_string()],
            ..AppConfig::default()
        };
        let active = ActiveExtensions::select(&config, &[skill("pdf")], &[], &[]);
        assert_eq!(active.skills.keys().collect::<Vec<_>>(), vec!["pdf"]);
        assert!(active.plugin_paths.is_empty());
        assert!(active.mcp_servers.is_empty());
    }

    #[test]
    fn select_lists_a_duplicated_definition_once() {
        let config = AppConfig {
            enabled_skills: vec!["pdf".to_string()],
            enabled_plugins: vec!["hud@official".to_string()],
            enabled_mcp_servers: vec!["files".to_string()],
            ..AppConfig::default()
        };
        let project_pdf =
            SkillDefinition { name: "pdf".to_string(), path: PathBuf::from("/project/pdf"), description: None };
        let active = ActiveExtensions::select(
            &config,
            &[skill("pdf"), project_pdf],
            &[plugin("hud", Some("official"), "/a"), plugin("hud", Some("official"), "/b")],
            &[server("files"), server("files")],
        );
        assert_eq!(active.skills.len(), 1);
        assert_eq!(active.skills["pdf"], PathBuf::from("/skills/pdf"), "the first definition wins");
        assert_eq!(active.plugin_paths, vec![PathBuf::from("/a")], "the first install wins");
        assert_eq!(active.mcp_servers.len(), 1);
    }

    #[test]
    fn select_with_nothing_enabled_is_empty() {
        let active = ActiveExtensions::select(&AppConfig::default(), &[skill("pdf")], &[], &[server("files")]);
        assert_eq!(active, ActiveExtensions::default());
    }
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p forge-domain extension`
Expected: compile error — `ActiveExtensions` and `PluginDefinition::key` not found.

- [ ] **Step 3: Implement**

At the top of `crates/forge-domain/src/extension.rs` replace `use std::path::PathBuf;` with:

```rust
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::config::AppConfig;
```

After the `McpServerDefinition` struct (before `#[cfg(test)]`) add:

```rust
impl PluginDefinition {
    /// The string `enabled_plugins` stores: `name@marketplace`, or just `name` without a marketplace.
    pub fn key(&self) -> String {
        match &self.marketplace {
            Some(marketplace) => format!("{}@{}", self.name, marketplace),
            None => self.name.clone(),
        }
    }
}

/// What a session is started with: the enabled skills (name to directory), the enabled plugins (by
/// install path) and the enabled MCP servers (name to the server's config, as read).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ActiveExtensions {
    pub skills: Vec<String>,
    pub plugin_paths: Vec<PathBuf>,
    pub mcp_servers: BTreeMap<String, serde_json::Value>,
}

impl ActiveExtensions {
    /// The definitions `config` has switched on. An enabled name with no definition is skipped, and a
    /// definition that appears twice (same skill name, same plugin installed under two scopes, same
    /// server name) is taken once — the first one, in the order given.
    pub fn select(
        config: &AppConfig,
        skills: &[SkillDefinition],
        plugins: &[PluginDefinition],
        servers: &[McpServerDefinition],
    ) -> Self {
        let mut active = ActiveExtensions::default();
        for skill in skills {
            if config.enabled_skills.contains(&skill.name) {
                active.skills.entry(skill.name.clone()).or_insert_with(|| skill.path.clone());
            }
        }
        let mut seen_plugins: Vec<String> = Vec::new();
        for plugin in plugins {
            let key = plugin.key();
            if config.enabled_plugins.contains(&key) && !seen_plugins.contains(&key) {
                active.plugin_paths.push(plugin.path.clone());
                seen_plugins.push(key);
            }
        }
        for server in servers {
            if config.enabled_mcp_servers.contains(&server.name) {
                active.mcp_servers.entry(server.name.clone()).or_insert_with(|| server.config.clone());
            }
        }
        active
    }
}
```

In `crates/forge-domain/src/lib.rs` change the extension export to:

```rust
pub use extension::{ActiveExtensions, McpServerDefinition, PluginDefinition, SkillDefinition};
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p forge-domain`
Expected: PASS (all, including the five new).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-domain
git commit -m "feat: add ActiveExtensions, the enabled set a session starts with

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Protocol — `StartSessionParams.extensions` and the wire fields

**Files:**
- Modify: `crates/forge-ports/src/sidecar_port.rs`
- Modify: `crates/forge-adapter-sidecar/src/protocol.rs`
- Modify: `crates/forge-adapter-sidecar/tests/fixtures/fake_sidecar.js`
- Modify: `crates/forge-adapter-sidecar/tests/contract_test.rs`
- Modify (add `extensions: Default::default()` to every `StartSessionParams` literal): `crates/forge-adapter-sidecar/tests/factory_test.rs`, `crates/forge-adapter-sidecar/examples/spike_drive_sidecar.rs`, `crates/forge-application/src/live_session.rs` (the `start_params` test helper), `crates/forge-application/src/live_session_actor.rs`, `crates/forge-application/src/use_cases/session_lifecycle.rs` (two literals), `crates/forge-ports/src/sidecar_port.rs` (its test)

**Interfaces:**
- Consumes: `forge_domain::ActiveExtensions` (Task 2).
- Produces:
  - `StartSessionParams { …existing, pub extensions: ActiveExtensions }`
  - the JSON `start_session` command gains `"skills": {name: directory}` (path strings), `"plugins": [..]` (absolute path strings) and `"mcp_servers": {name: config}`, always present.

- [ ] **Step 1: Write the failing tests**

In `crates/forge-adapter-sidecar/src/protocol.rs`, update the existing test's `StartSessionParams` literal to include `extensions: ActiveExtensions::default(),`, its expected JSON to add `"skills": {}, "plugins": [], "mcp_servers": {}`, add these imports to the `tests` module (`use forge_domain::ActiveExtensions; use std::collections::BTreeMap; use std::path::PathBuf;`), and add:

```rust
    #[test]
    fn start_session_command_carries_the_active_extensions() {
        let mut mcp_servers = BTreeMap::new();
        mcp_servers.insert("files".to_string(), serde_json::json!({"command": "npx", "args": ["files-mcp"]}));
        let command = SidecarCommand::start_session(
            "1".to_string(),
            StartSessionParams {
                cwd: "/tmp/project".to_string(),
                resume_session_id: None,
                permission_mode: PermissionMode::Default,
                model: None,
                extensions: ActiveExtensions {
                    skills: BTreeMap::from([("pdf".to_string(), PathBuf::from("/skills/pdf"))]),
                    plugin_paths: vec![PathBuf::from("/plugins/hud")],
                    mcp_servers,
                },
            },
        );

        let json = serde_json::to_value(&command).unwrap();
        assert_eq!(json["skills"], serde_json::json!({"pdf": "/skills/pdf"}));
        assert_eq!(json["plugins"], serde_json::json!(["/plugins/hud"]));
        assert_eq!(json["mcp_servers"], serde_json::json!({"files": {"command": "npx", "args": ["files-mcp"]}}));
    }
```

In `crates/forge-adapter-sidecar/tests/contract_test.rs` add `extensions: Default::default(),` to `default_params()`, add `use forge_domain::ActiveExtensions; use std::collections::BTreeMap; use std::path::PathBuf;` at the top, and append:

```rust
#[tokio::test]
async fn start_session_delivers_the_active_extensions_to_the_sidecar() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &[]).unwrap();
    let mut params = default_params();
    params.extensions = ActiveExtensions {
        skills: BTreeMap::from([("pdf".to_string(), PathBuf::from("/skills/pdf"))]),
        plugin_paths: vec![PathBuf::from("/plugins/hud")],
        mcp_servers: BTreeMap::from([("files".to_string(), serde_json::json!({"command": "npx"}))]),
    };
    sidecar.start_session(params).await.unwrap();
    sidecar.next_event().await.unwrap();

    sidecar.send_message("TRIGGER_EXTENSIONS".to_string()).await.unwrap();
    match sidecar.next_event().await.unwrap() {
        SidecarEvent::AssistantDelta { text } => {
            let seen: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(
                seen,
                serde_json::json!({
                    "skills": {"pdf": "/skills/pdf"},
                    "plugins": ["/plugins/hud"],
                    "mcp_servers": {"files": {"command": "npx"}}
                })
            );
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn a_session_without_extensions_still_sends_empty_lists() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &[]).unwrap();
    sidecar.start_session(default_params()).await.unwrap();
    sidecar.next_event().await.unwrap();

    sidecar.send_message("TRIGGER_EXTENSIONS".to_string()).await.unwrap();
    match sidecar.next_event().await.unwrap() {
        SidecarEvent::AssistantDelta { text } => {
            let seen: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(seen, serde_json::json!({"skills": {}, "plugins": [], "mcp_servers": {}}));
        }
        other => panic!("unexpected event: {other:?}"),
    }
}
```

In `crates/forge-adapter-sidecar/tests/fixtures/fake_sidecar.js`: after the `emit` function add `let started = null;`; change the `start_session` case to

```js
    case "start_session":
      started = command;
      emit({ type: "session_started", session_id: "fake-session-1" });
      return;
```

and, just before the `if (command.text.startsWith("TRIGGER_ENV:")) {` block, add:

```js
      if (command.text === "TRIGGER_EXTENSIONS") {
        emit({
          type: "assistant_delta",
          text: JSON.stringify({ skills: started.skills, plugins: started.plugins, mcp_servers: started.mcp_servers }),
        });
        return;
      }
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p forge-adapter-sidecar`
Expected: compile error — `StartSessionParams` has no field `extensions`.

- [ ] **Step 3: Implement**

`crates/forge-ports/src/sidecar_port.rs`: change the import to `use forge_domain::{ActiveExtensions, PermissionDecision, PermissionMode};`, add the field, and fix the test literal (`extensions: ActiveExtensions::default(),`):

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct StartSessionParams {
    pub cwd: String,
    pub resume_session_id: Option<String>,
    pub permission_mode: PermissionMode,
    pub model: Option<String>,
    /// The skills, plugins and MCP servers the session starts with.
    pub extensions: ActiveExtensions,
}
```

`crates/forge-adapter-sidecar/src/protocol.rs`: add `use std::collections::BTreeMap;` at the top, extend the variant and constructor:

```rust
    StartSession {
        id: String,
        cwd: String,
        resume_session_id: Option<String>,
        permission_mode: WireMode,
        model: Option<String>,
        skills: BTreeMap<String, String>,
        plugins: Vec<String>,
        mcp_servers: BTreeMap<String, serde_json::Value>,
    },
```

```rust
    pub fn start_session(id: String, params: StartSessionParams) -> Self {
        SidecarCommand::StartSession {
            id,
            cwd: params.cwd,
            resume_session_id: params.resume_session_id,
            permission_mode: params.permission_mode.into(),
            model: params.model,
            skills: params
                .extensions
                .skills
                .into_iter()
                .map(|(name, dir)| (name, dir.to_string_lossy().to_string()))
                .collect(),
            plugins: params
                .extensions
                .plugin_paths
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect(),
            mcp_servers: params.extensions.mcp_servers,
        }
    }
```

Then add `extensions: Default::default(),` to the remaining literals listed under **Files** (production ones in `live_session_actor.rs` and `session_lifecycle.rs` use `Default::default()` for now — Task 6 replaces the actor's).

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --workspace`
Expected: PASS (workspace compiles again; the two new contract tests and the protocol test pass).

- [ ] **Step 5: Commit**

```bash
git add -A crates
git commit -m "feat: carry the active skills, plugins and MCP servers in start_session

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Sidecar — map the fields to SDK options and build the `forge-skills` plugin

**Files:**
- Create: `sidecar/src/skills_plugin.ts`
- Modify: `sidecar/src/protocol.ts`
- Modify: `sidecar/src/session.ts`
- Modify: `sidecar/test/session.test.ts`, `sidecar/test/protocol.test.ts`

**Interfaces:**
- Consumes: the `start_session` JSON of Task 3 (`skills` is `{name: directory}`); spec §17.7: `settingSources: ["project"]`, and user skills reach the session through a generated local plugin (a user skill is not discovered under `["project"]`; `["user","project"]` leaks the user's plugins).
- Produces:
  - `skills_plugin.ts`: `SKILLS_PLUGIN_NAME = "forge-skills"`, `isSafeSkillName(name: string): boolean`, `buildSkillsPlugin(skills: Record<string, string>): Promise<SkillsPlugin | null>` where `SkillsPlugin = { path: string; skills: string[]; cleanup: () => Promise<void> }` (`skills` are the `forge-skills:<name>` entries; `null` when no safe skill is given).
  - `query()` options `settingSources`, `strictMcpConfig: true`, `skills`, `plugins`, `mcpServers` — always present.

- [ ] **Step 1: Write the failing tests**

In `sidecar/test/session.test.ts` add imports `import { existsSync, lstatSync, mkdtempSync, mkdirSync, readFileSync, readlinkSync, writeFileSync } from "node:fs"; import { tmpdir } from "node:os"; import { join } from "node:path";`, update `startCommand` to include the new fields, and add the tests inside `describe("SidecarSession", …)`:

```ts
const startCommand: SidecarCommand = {
  type: "start_session",
  id: "1",
  cwd: "/tmp",
  resume_session_id: null,
  permission_mode: "default",
  model: null,
  skills: {},
  plugins: [],
  mcp_servers: {},
};

function makeSkillDir(name: string): string {
  const dir = join(mkdtempSync(join(tmpdir(), "forge-test-skill-")), name);
  mkdirSync(dir);
  writeFileSync(join(dir, "SKILL.md"), `---\nname: ${name}\ndescription: test\n---\n`);
  return dir;
}
```

```ts
  it("starts the SDK with only the project settings source and exactly the enabled extensions", async () => {
    let captured: Record<string, unknown> | undefined;
    const queryFn: QueryFn = ({ options }) => {
      captured = options;
      return toQueryHandle((async function* () {})());
    };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle({
      ...startCommand,
      plugins: ["/plugins/hud"],
      mcp_servers: { files: { command: "npx", args: ["files-mcp"] } },
    });

    expect(captured).toMatchObject({
      settingSources: ["project"],
      strictMcpConfig: true,
      skills: [],
      plugins: [{ type: "local", path: "/plugins/hud" }],
      mcpServers: { files: { command: "npx", args: ["files-mcp"] } },
    });
  });

  it("passes empty lists instead of omitting them, so the SDK defaults do not come back", async () => {
    let captured: Record<string, unknown> | undefined;
    const queryFn: QueryFn = ({ options }) => {
      captured = options;
      return toQueryHandle((async function* () {})());
    };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle(startCommand);

    expect(captured?.skills).toEqual([]);
    expect(captured?.plugins).toEqual([]);
    expect(captured?.mcpServers).toEqual({});
  });

  it("treats a command that lacks the extension fields as nothing enabled", async () => {
    let captured: Record<string, unknown> | undefined;
    const queryFn: QueryFn = ({ options }) => {
      captured = options;
      return toQueryHandle((async function* () {})());
    };
    const oldCommand = { type: "start_session", id: "1", cwd: "/tmp", resume_session_id: null, permission_mode: "default", model: null };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle(oldCommand as unknown as SidecarCommand);

    expect(captured?.skills).toEqual([]);
    expect(captured?.plugins).toEqual([]);
    expect(captured?.mcpServers).toEqual({});
  });

  it("delivers enabled skills through a generated forge-skills plugin and removes it when the session ends", async () => {
    const pdfDir = makeSkillDir("pdf");
    let pluginPath = "";
    let seen: { manifest: string; link: string } | undefined;
    let captured: Record<string, unknown> | undefined;
    const queryFn: QueryFn = ({ options }) => {
      captured = options;
      const plugins = options.plugins as { type: string; path: string }[];
      pluginPath = plugins[plugins.length - 1].path;
      seen = {
        manifest: readFileSync(join(pluginPath, ".claude-plugin", "plugin.json"), "utf8"),
        link: readlinkSync(join(pluginPath, "skills", "pdf")),
      };
      return toQueryHandle((async function* () {})());
    };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle({ ...startCommand, skills: { pdf: pdfDir }, plugins: ["/plugins/hud"] });

    expect(captured?.skills).toEqual(["forge-skills:pdf"]);
    expect((captured?.plugins as { path: string }[])[0].path).toBe("/plugins/hud");
    expect(JSON.parse(seen!.manifest)).toEqual({ name: "forge-skills" });
    expect(seen!.link).toBe(pdfDir);
    expect(existsSync(pluginPath)).toBe(false);
    expect(existsSync(join(pdfDir, "SKILL.md"))).toBe(true);
  });

  it("skips a skill whose name is not a safe path component", async () => {
    const okDir = makeSkillDir("ok");
    let captured: Record<string, unknown> | undefined;
    const queryFn: QueryFn = ({ options }) => {
      captured = options;
      return toQueryHandle((async function* () {})());
    };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle({
      ...startCommand,
      skills: { "../evil": okDir, "a/b": okDir, "..": okDir, ok: okDir },
    });

    expect(captured?.skills).toEqual(["forge-skills:ok"]);
  });

  it("generates no plugin when every skill name is unsafe", async () => {
    const okDir = makeSkillDir("ok");
    let captured: Record<string, unknown> | undefined;
    const queryFn: QueryFn = ({ options }) => {
      captured = options;
      return toQueryHandle((async function* () {})());
    };

    const session = new SidecarSession(queryFn, () => {});
    await session.handle({ ...startCommand, skills: { "../evil": okDir } });

    expect(captured?.skills).toEqual([]);
    expect(captured?.plugins).toEqual([]);
  });
```

In `sidecar/test/protocol.test.ts` add to `describe("parseCommand", …)`:

```ts
  it("keeps the extension fields of a start_session command", () => {
    const line = JSON.stringify({
      type: "start_session",
      id: "1",
      cwd: "/tmp",
      resume_session_id: null,
      permission_mode: "default",
      model: null,
      skills: { pdf: "/skills/pdf" },
      plugins: ["/plugins/hud"],
      mcp_servers: { files: { command: "npx" } },
    });
    expect(parseCommand(line)).toMatchObject({
      skills: { pdf: "/skills/pdf" },
      plugins: ["/plugins/hud"],
      mcp_servers: { files: { command: "npx" } },
    });
  });
```

(`lstatSync` in the import list is only needed if the implementer uses it; drop unused imports so `npm run typecheck` stays clean.)

- [ ] **Step 2: Run the tests to see them fail**

Run: `cd sidecar && npm test`
Expected: FAIL — `captured` has no `settingSources`; there is no generated plugin.

- [ ] **Step 3: Implement**

Create `sidecar/src/skills_plugin.ts`:

```ts
import { mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

export const SKILLS_PLUGIN_NAME = "forge-skills";

export interface SkillsPlugin {
  /** The generated plugin directory, to pass to the SDK as a local plugin. */
  path: string;
  /** The `forge-skills:<name>` entries for the SDK's `skills` option. */
  skills: string[];
  /** Removes the generated directory (the symlinks, never what they point to). */
  cleanup: () => Promise<void>;
}

// A name becomes a directory entry, so it must be exactly one safe path component.
export function isSafeSkillName(name: string): boolean {
  return name.length > 0 && name !== "." && name !== ".." && !/[\\/\0]/.test(name);
}

// The SDK does not discover user skills under settingSources ['project'], and ['user', 'project']
// leaks the user's enabled plugins (spec §17.7). A local plugin is loaded either way, so the enabled
// skills are gathered into one as symlinks. Returns null when there is nothing safe to gather.
export async function buildSkillsPlugin(skills: Record<string, string>): Promise<SkillsPlugin | null> {
  const names = Object.keys(skills).filter(isSafeSkillName);
  if (names.length === 0) return null;
  const root = await mkdtemp(join(tmpdir(), "forge-skills-"));
  try {
    await mkdir(join(root, ".claude-plugin"));
    await writeFile(join(root, ".claude-plugin", "plugin.json"), JSON.stringify({ name: SKILLS_PLUGIN_NAME }));
    await mkdir(join(root, "skills"));
    for (const name of names) {
      await symlink(skills[name], join(root, "skills", name), "dir");
    }
  } catch (err) {
    await rm(root, { recursive: true, force: true });
    throw err;
  }
  return {
    path: root,
    skills: names.map((name) => `${SKILLS_PLUGIN_NAME}:${name}`),
    cleanup: () => rm(root, { recursive: true, force: true }),
  };
}
```

`sidecar/src/protocol.ts`: extend the `start_session` variant:

```ts
  | {
      type: "start_session";
      id: string;
      cwd: string;
      resume_session_id: string | null;
      permission_mode: WirePermissionMode;
      model: string | null;
      skills: Record<string, string>;
      plugins: string[];
      mcp_servers: Record<string, unknown>;
    }
```

`sidecar/src/session.ts`: `import { buildSkillsPlugin } from "./skills_plugin.js";`. In `start`, before `const query = this.queryFn(…)`, add

```ts
    // Forge's toggles are the only source of truth (spec §17.1). The `??` guards a command from an
    // older parent that omits the fields: that must mean nothing enabled, never the SDK's defaults.
    const skillsPlugin = await buildSkillsPlugin(command.skills ?? {});
    const plugins = [
      ...(command.plugins ?? []).map((path) => ({ type: "local" as const, path })),
      ...(skillsPlugin ? [{ type: "local" as const, path: skillsPlugin.path }] : []),
    ];
```

add to the `options` object after `includePartialMessages: true,`:

```ts
        settingSources: ["project"],
        strictMcpConfig: true,
        skills: skillsPlugin?.skills ?? [],
        plugins,
        mcpServers: command.mcp_servers ?? {},
```

and make the generated directory go away when the session ends: at the end of the `this.sessionEndedPromise = (async () => { try { … } catch (err) { … } })();` block add a `finally { await skillsPlugin?.cleanup(); }` after the `catch`. If `queryFn` itself throws synchronously, call `await skillsPlugin?.cleanup()` before rethrowing (wrap the `this.queryFn(…)` call in a try/catch that does that).

- [ ] **Step 4: Run the tests and typecheck**

Run: `cd sidecar && npm test && npm run typecheck`
Expected: PASS, no type errors.

- [ ] **Step 5: Commit**

```bash
git add sidecar
git commit -m "feat: start SDK sessions with only Forge's enabled extensions, skills via a generated plugin

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Application — `ExtensionsUseCase`

**Files:**
- Create: `crates/forge-application/src/use_cases/extensions.rs`
- Modify: `crates/forge-application/src/use_cases/mod.rs`
- Modify: `crates/forge-application/src/lib.rs`

**Interfaces:**
- Consumes: `ConfigRepositoryPort`, `SkillRepositoryPort`, `PluginRepositoryPort`, `McpRegistryPort`; `ActiveExtensions::select` (Task 2); the existing `ToggleSkillUseCase` / `TogglePluginUseCase` / `ToggleMcpServerUseCase`.
- Produces:
  - `ExtensionsUseCase::new(config: Arc<dyn ConfigRepositoryPort>, skills: Arc<dyn SkillRepositoryPort>, plugins: Arc<dyn PluginRepositoryPort>, mcp: Arc<dyn McpRegistryPort>, global_scope: PathBuf) -> Self` — `global_scope` is the directory read as "the project" when none is active (the home directory, so only global definitions show).
  - `async fn resolve_active(&self, project: &Path) -> Result<ActiveExtensions, ExtensionsError>`
  - `async fn load_config(&self) -> Result<AppConfig, ConfigError>`
  - `async fn list_skills(&self, project: Option<&Path>) -> Result<Vec<SkillDefinition>, SkillRepositoryError>`
  - `async fn list_plugins(&self) -> Result<Vec<PluginDefinition>, PluginRepositoryError>`
  - `async fn list_mcp_servers(&self, project: Option<&Path>) -> Result<Vec<McpServerDefinition>, McpRegistryError>`
  - `async fn toggle_skill(&self, name: &str)`, `toggle_plugin(&self, key: &str)`, `toggle_mcp_server(&self, name: &str)` — each `Result<AppConfig, ConfigError>`
  - `enum ExtensionsError { Config(ConfigError), Skills(SkillRepositoryError), Plugins(PluginRepositoryError), Mcp(McpRegistryError) }` with `Display`.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-application/src/use_cases/extensions.rs` containing only the tests first (the implementation goes in Step 3, above them):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{McpServerDefinition, PluginDefinition, SkillDefinition};
    use forge_ports::{
        MockConfigRepositoryPort, MockMcpRegistryPort, MockPluginRepositoryPort, MockSkillRepositoryPort,
    };

    fn skill(name: &str) -> SkillDefinition {
        SkillDefinition { name: name.to_string(), path: PathBuf::from(format!("/skills/{name}")), description: None }
    }

    fn config_returning(config: AppConfig) -> MockConfigRepositoryPort {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().returning(move || Ok(config.clone()));
        mock
    }

    fn use_case(
        config: MockConfigRepositoryPort,
        skills: MockSkillRepositoryPort,
        plugins: MockPluginRepositoryPort,
        mcp: MockMcpRegistryPort,
    ) -> ExtensionsUseCase {
        ExtensionsUseCase::new(
            Arc::new(config),
            Arc::new(skills),
            Arc::new(plugins),
            Arc::new(mcp),
            PathBuf::from("/home/me"),
        )
    }

    #[tokio::test]
    async fn resolve_active_keeps_only_what_is_enabled() {
        let config = AppConfig {
            enabled_skills: vec!["pdf".to_string()],
            enabled_plugins: vec!["hud@official".to_string()],
            enabled_mcp_servers: vec!["files".to_string()],
            ..AppConfig::default()
        };
        let mut skills = MockSkillRepositoryPort::new();
        skills.expect_list_skills().times(1).returning(|_| Ok(vec![skill("pdf"), skill("docx")]));
        let mut plugins = MockPluginRepositoryPort::new();
        plugins.expect_list_plugins().times(1).returning(|| {
            Ok(vec![PluginDefinition {
                name: "hud".to_string(),
                marketplace: Some("official".to_string()),
                path: PathBuf::from("/plugins/hud"),
            }])
        });
        let mut mcp = MockMcpRegistryPort::new();
        mcp.expect_list_servers().times(1).returning(|_| {
            Ok(vec![McpServerDefinition { name: "files".to_string(), config: serde_json::json!({"command": "npx"}) }])
        });

        let active = use_case(config_returning(config), skills, plugins, mcp)
            .resolve_active(Path::new("/work/a"))
            .await
            .unwrap();

        assert_eq!(active.skills.keys().collect::<Vec<_>>(), vec!["pdf"]);
        assert_eq!(active.plugin_paths, vec![PathBuf::from("/plugins/hud")]);
        assert_eq!(active.mcp_servers["files"]["command"], "npx");
    }

    #[tokio::test]
    async fn resolve_active_with_nothing_enabled_reads_no_definitions() {
        // The reader mocks have no expectations: any call would panic.
        let active = use_case(
            config_returning(AppConfig::default()),
            MockSkillRepositoryPort::new(),
            MockPluginRepositoryPort::new(),
            MockMcpRegistryPort::new(),
        )
        .resolve_active(Path::new("/work/a"))
        .await
        .unwrap();

        assert_eq!(active, ActiveExtensions::default());
    }

    #[tokio::test]
    async fn resolve_active_only_reads_the_kinds_that_have_something_enabled() {
        let config = AppConfig { enabled_skills: vec!["pdf".to_string()], ..AppConfig::default() };
        let mut skills = MockSkillRepositoryPort::new();
        skills.expect_list_skills().times(1).returning(|_| Ok(vec![skill("pdf")]));

        let active = use_case(
            config_returning(config),
            skills,
            MockPluginRepositoryPort::new(),
            MockMcpRegistryPort::new(),
        )
        .resolve_active(Path::new("/work/a"))
        .await
        .unwrap();

        assert_eq!(active.skills.keys().collect::<Vec<_>>(), vec!["pdf"]);
    }

    #[tokio::test]
    async fn resolve_active_fails_when_a_needed_reader_fails() {
        let config = AppConfig { enabled_skills: vec!["pdf".to_string()], ..AppConfig::default() };
        let mut skills = MockSkillRepositoryPort::new();
        skills
            .expect_list_skills()
            .returning(|_| Err(SkillRepositoryError::ListFailed("boom".to_string())));

        let error = use_case(
            config_returning(config),
            skills,
            MockPluginRepositoryPort::new(),
            MockMcpRegistryPort::new(),
        )
        .resolve_active(Path::new("/work/a"))
        .await
        .unwrap_err();

        assert!(matches!(error, ExtensionsError::Skills(_)));
        assert!(error.to_string().contains("boom"), "{error}");
    }

    #[tokio::test]
    async fn resolve_active_fails_when_the_config_cannot_be_read() {
        let mut config = MockConfigRepositoryPort::new();
        config.expect_load().returning(|| Err(ConfigError::ParseFailed("bad json".to_string())));

        let error = use_case(
            config,
            MockSkillRepositoryPort::new(),
            MockPluginRepositoryPort::new(),
            MockMcpRegistryPort::new(),
        )
        .resolve_active(Path::new("/work/a"))
        .await
        .unwrap_err();

        assert!(matches!(error, ExtensionsError::Config(_)));
    }

    #[tokio::test]
    async fn listing_without_a_project_reads_the_global_scope() {
        let mut skills = MockSkillRepositoryPort::new();
        skills
            .expect_list_skills()
            .withf(|path| path == Path::new("/home/me"))
            .times(1)
            .returning(|_| Ok(vec![]));
        let mut mcp = MockMcpRegistryPort::new();
        mcp.expect_list_servers()
            .withf(|path| path == Path::new("/work/a"))
            .times(1)
            .returning(|_| Ok(vec![]));

        let extensions = use_case(
            config_returning(AppConfig::default()),
            skills,
            MockPluginRepositoryPort::new(),
            mcp,
        );
        extensions.list_skills(None).await.unwrap();
        extensions.list_mcp_servers(Some(Path::new("/work/a"))).await.unwrap();
    }

    #[tokio::test]
    async fn toggling_a_plugin_stores_its_key_and_keeps_the_other_lists() {
        let mut config = MockConfigRepositoryPort::new();
        config.expect_load().returning(|| {
            Ok(AppConfig { enabled_skills: vec!["pdf".to_string()], ..AppConfig::default() })
        });
        config
            .expect_save()
            .withf(|c: &AppConfig| {
                c.enabled_plugins == vec!["hud@official".to_string()] && c.enabled_skills == vec!["pdf".to_string()]
            })
            .times(1)
            .returning(|_| Ok(()));

        let saved = use_case(
            config,
            MockSkillRepositoryPort::new(),
            MockPluginRepositoryPort::new(),
            MockMcpRegistryPort::new(),
        )
        .toggle_plugin("hud@official")
        .await
        .unwrap();

        assert_eq!(saved.enabled_plugins, vec!["hud@official".to_string()]);
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

Register the module first so it compiles as a unit: in `use_cases/mod.rs` add `mod extensions;` and `pub use extensions::{ExtensionsError, ExtensionsUseCase};`; in `lib.rs` add `ExtensionsError, ExtensionsUseCase` to the `pub use use_cases::{…}` list.

Run: `cargo test -p forge-application extensions`
Expected: compile error — `ExtensionsUseCase` not found.

- [ ] **Step 3: Implement**

Put this at the top of `extensions.rs`, above the `tests` module:

```rust
use std::path::{Path, PathBuf};
use std::sync::Arc;

use forge_domain::{ActiveExtensions, AppConfig, McpServerDefinition, PluginDefinition, SkillDefinition};
use forge_ports::{
    ConfigError, ConfigRepositoryPort, McpRegistryError, McpRegistryPort, PluginRepositoryError,
    PluginRepositoryPort, SkillRepositoryError, SkillRepositoryPort,
};
use thiserror::Error;

use super::toggles::{ToggleMcpServerUseCase, TogglePluginUseCase, ToggleSkillUseCase};

#[derive(Debug, Error)]
pub enum ExtensionsError {
    #[error("failed to read the Forge config: {0}")]
    Config(#[from] ConfigError),
    #[error("{0}")]
    Skills(#[from] SkillRepositoryError),
    #[error("{0}")]
    Plugins(#[from] PluginRepositoryError),
    #[error("{0}")]
    Mcp(#[from] McpRegistryError),
}

/// Everything about skills, plugins and MCP servers that the app needs: what a new session starts
/// with, what the settings tabs list, and the toggles. Definitions are read from the shared
/// `~/.claude` locations; which of them are on is Forge's own `enabled_*` lists (spec §4.3).
pub struct ExtensionsUseCase {
    config: Arc<dyn ConfigRepositoryPort>,
    skills: Arc<dyn SkillRepositoryPort>,
    plugins: Arc<dyn PluginRepositoryPort>,
    mcp: Arc<dyn McpRegistryPort>,
    /// Read as "the project" when no project is active, so only global definitions show.
    global_scope: PathBuf,
}

impl ExtensionsUseCase {
    pub fn new(
        config: Arc<dyn ConfigRepositoryPort>,
        skills: Arc<dyn SkillRepositoryPort>,
        plugins: Arc<dyn PluginRepositoryPort>,
        mcp: Arc<dyn McpRegistryPort>,
        global_scope: PathBuf,
    ) -> Self {
        Self { config, skills, plugins, mcp, global_scope }
    }

    /// The enabled set for a session in `project`. A reader is only called when something of its
    /// kind is enabled, so a broken reader cannot block sessions for someone who uses none; when
    /// one is needed and fails, so does the start (a session never starts with fewer tools than
    /// were switched on).
    pub async fn resolve_active(&self, project: &Path) -> Result<ActiveExtensions, ExtensionsError> {
        let config = self.config.load().await?;
        let skills = if config.enabled_skills.is_empty() {
            Vec::new()
        } else {
            self.skills.list_skills(project).await?
        };
        let plugins = if config.enabled_plugins.is_empty() {
            Vec::new()
        } else {
            self.plugins.list_plugins().await?
        };
        let servers = if config.enabled_mcp_servers.is_empty() {
            Vec::new()
        } else {
            self.mcp.list_servers(project).await?
        };
        Ok(ActiveExtensions::select(&config, &skills, &plugins, &servers))
    }

    pub async fn load_config(&self) -> Result<AppConfig, ConfigError> {
        self.config.load().await
    }

    pub async fn list_skills(&self, project: Option<&Path>) -> Result<Vec<SkillDefinition>, SkillRepositoryError> {
        self.skills.list_skills(project.unwrap_or(&self.global_scope)).await
    }

    pub async fn list_plugins(&self) -> Result<Vec<PluginDefinition>, PluginRepositoryError> {
        self.plugins.list_plugins().await
    }

    pub async fn list_mcp_servers(
        &self,
        project: Option<&Path>,
    ) -> Result<Vec<McpServerDefinition>, McpRegistryError> {
        self.mcp.list_servers(project.unwrap_or(&self.global_scope)).await
    }

    pub async fn toggle_skill(&self, name: &str) -> Result<AppConfig, ConfigError> {
        ToggleSkillUseCase::new(self.config.clone()).execute(name).await
    }

    /// `key` is `PluginDefinition::key()`.
    pub async fn toggle_plugin(&self, key: &str) -> Result<AppConfig, ConfigError> {
        TogglePluginUseCase::new(self.config.clone()).execute(key).await
    }

    pub async fn toggle_mcp_server(&self, name: &str) -> Result<AppConfig, ConfigError> {
        ToggleMcpServerUseCase::new(self.config.clone()).execute(name).await
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p forge-application`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-application
git commit -m "feat: add ExtensionsUseCase to resolve, list and toggle skills, plugins and MCP servers

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Resolve at sidecar start, and wire the composition root

**Files:**
- Modify: `crates/forge-application/src/test_support.rs`
- Modify: `crates/forge-application/src/live_session.rs`, `live_session_actor.rs`, `registry.rs`
- Modify: `crates/forge-features/src/workspace.rs` (`WorkspaceServices`)
- Modify: `crates/forge-app/src/main.rs`

**Interfaces:**
- Consumes: `ExtensionsUseCase` (Task 5); `StartSessionParams.extensions` (Task 3).
- Produces:
  - `LiveSessionInit.extensions: Arc<ExtensionsUseCase>`
  - `SessionRegistry::new(factory, sessions, extensions: Arc<ExtensionsUseCase>)`
  - `WorkspaceServices.extensions: Arc<ExtensionsUseCase>`
  - test helpers in `test_support`: `extensions_with(config: AppConfig, skills: Vec<SkillDefinition>) -> Arc<ExtensionsUseCase>`, `no_extensions() -> Arc<ExtensionsUseCase>`, `failing_extensions() -> Arc<ExtensionsUseCase>` (config enables a skill and the skill reader errors with "skill reader is down").

- [ ] **Step 1: Add the test helpers and write the failing tests**

Append to `crates/forge-application/src/test_support.rs` (add the imports it lacks: `use std::path::PathBuf; use forge_domain::{AppConfig, SkillDefinition}; use forge_ports::{MockConfigRepositoryPort, MockMcpRegistryPort, MockPluginRepositoryPort, MockSkillRepositoryPort, SkillRepositoryError}; use crate::use_cases::ExtensionsUseCase;`):

```rust
/// An `ExtensionsUseCase` over mocks: `config` is what the config file holds, `skills` what the
/// skill reader finds. The plugin and MCP readers have no expectations, so a test that needs them
/// gets a panic rather than a silent empty answer.
pub(crate) fn extensions_with(config: AppConfig, skills: Vec<SkillDefinition>) -> Arc<ExtensionsUseCase> {
    let mut config_port = MockConfigRepositoryPort::new();
    config_port.expect_load().returning(move || Ok(config.clone()));
    let mut skill_port = MockSkillRepositoryPort::new();
    skill_port.expect_list_skills().returning(move |_| Ok(skills.clone()));
    Arc::new(ExtensionsUseCase::new(
        Arc::new(config_port),
        Arc::new(skill_port),
        Arc::new(MockPluginRepositoryPort::new()),
        Arc::new(MockMcpRegistryPort::new()),
        PathBuf::from("/home/me"),
    ))
}

/// Nothing enabled, so no reader is ever called.
pub(crate) fn no_extensions() -> Arc<ExtensionsUseCase> {
    extensions_with(AppConfig::default(), Vec::new())
}

/// A skill is enabled and the skill reader fails.
pub(crate) fn failing_extensions() -> Arc<ExtensionsUseCase> {
    let mut config_port = MockConfigRepositoryPort::new();
    config_port
        .expect_load()
        .returning(|| Ok(AppConfig { enabled_skills: vec!["pdf".to_string()], ..AppConfig::default() }));
    let mut skill_port = MockSkillRepositoryPort::new();
    skill_port
        .expect_list_skills()
        .returning(|_| Err(SkillRepositoryError::ListFailed("skill reader is down".to_string())));
    Arc::new(ExtensionsUseCase::new(
        Arc::new(config_port),
        Arc::new(skill_port),
        Arc::new(MockPluginRepositoryPort::new()),
        Arc::new(MockMcpRegistryPort::new()),
        PathBuf::from("/home/me"),
    ))
}
```

In `crates/forge-application/src/live_session.rs` tests: import `no_extensions, extensions_with, failing_extensions` from `crate::test_support`, add `extensions: no_extensions(),` to `new_init`, `extensions: ActiveExtensions::default(),` to `start_params` (import `forge_domain::{ActiveExtensions, AppConfig, SkillDefinition}` and `std::collections::BTreeMap`), and add:

```rust
    #[tokio::test]
    async fn the_started_session_gets_exactly_the_enabled_extensions() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let config = AppConfig { enabled_skills: vec!["pdf".to_string()], ..AppConfig::default() };
        let skills = vec![
            SkillDefinition { name: "pdf".to_string(), path: PathBuf::from("/skills/pdf"), description: None },
            SkillDefinition { name: "docx".to_string(), path: PathBuf::from("/skills/docx"), description: None },
        ];
        let mut init = new_init(factory);
        init.extensions = extensions_with(config, skills);
        let handle = spawn_live_session(init);
        let mut view = handle.subscribe();

        handle.send("hello".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        let mut expected = start_params(None);
        expected.extensions = ActiveExtensions {
            skills: BTreeMap::from([("pdf".to_string(), PathBuf::from("/skills/pdf"))]),
            ..ActiveExtensions::default()
        };
        assert_eq!(control.calls(), vec![Call::Start(expected), Call::Send("hello".to_string())]);
    }

    #[tokio::test]
    async fn a_reader_failure_spawns_nothing_and_keeps_the_typed_text() {
        let (sidecar, control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(sidecar)]);
        let mut init = new_init(factory);
        init.extensions = failing_extensions();
        let handle = spawn_live_session(init);
        let mut view = handle.subscribe();

        handle.send("hello".to_string());
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert!(seen.last_error.unwrap().contains("skill reader is down"));
        assert_eq!(seen.unsent.as_deref(), Some("hello"));
        assert_eq!(spawns.load(Ordering::SeqCst), 0, "no process for a session that cannot start");
        assert!(control.calls().is_empty());
    }
```

In `registry.rs` tests change the `registry` helper to `SessionRegistry::new(factory, sessions, crate::test_support::no_extensions())`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p forge-application`
Expected: compile errors — `LiveSessionInit` has no field `extensions`; `SessionRegistry::new` takes two arguments.

- [ ] **Step 3: Implement**

`live_session.rs`: add `use crate::use_cases::ExtensionsUseCase;`, the field `pub extensions: Arc<ExtensionsUseCase>,` in `LiveSessionInit`, and pass it: `Actor::new(view, view_tx, command_rx, init.factory, init.model, init.extensions)`.

`live_session_actor.rs`: import `crate::use_cases::ExtensionsUseCase`; add `extensions: Arc<ExtensionsUseCase>,` to `Actor` and as the last `new` parameter (store it); replace `start_sidecar` with:

```rust
    // Resolves what is switched on, spawns a sidecar and starts (or resumes) the session on it.
    // Resolving comes first so a failing reader spawns no process. The sidecar is only kept once
    // both steps succeed; otherwise it is dropped, which ends its process.
    async fn start_sidecar(&mut self) -> Result<(), String> {
        let extensions = self
            .extensions
            .resolve_active(&self.view.project_path)
            .await
            .map_err(|e| e.to_string())?;
        let sidecar = self.factory.spawn().await.map_err(|e| e.to_string())?;
        sidecar
            .start_session(StartSessionParams {
                cwd: self.view.project_path.to_string_lossy().to_string(),
                resume_session_id: self.view.claude_session_id.as_ref().map(|id| id.0.clone()),
                permission_mode: self.view.session.permission_mode,
                model: self.model.clone(),
                extensions,
            })
            .await
            .map_err(|e| e.to_string())?;
        self.sidecar = Some(sidecar);
        Ok(())
    }
```

`registry.rs`: add `extensions: Arc<ExtensionsUseCase>` to the struct, `new(factory, sessions, extensions)`, and `extensions: self.extensions.clone(),` in the `LiveSessionInit` built by `insert` (import `crate::use_cases::ExtensionsUseCase`).

`crates/forge-features/src/workspace.rs`: import `ExtensionsUseCase` from `forge_application` and add `pub extensions: Arc<ExtensionsUseCase>,` to `WorkspaceServices`.

`crates/forge-app/src/main.rs`: change the imports to

```rust
use forge_adapter_claude_fs::{
    default_claude_home, default_claude_json_path, ClaudeMcpAdapter, ClaudePluginAdapter, ClaudeProjectAdapter,
    ClaudeSessionAdapter, ClaudeSkillAdapter,
};
use forge_application::{ExtensionsUseCase, SessionRegistry, SettingsUseCase};
```

and in `build_services` (after `let settings = …`):

```rust
    let claude_json = default_claude_json_path().expect("could not resolve the home directory");
    // With no project open the tabs read the home directory as the project, so only the global
    // definitions (`~/.claude/skills`, the global MCP servers) show.
    let home = claude_home.parent().map(std::path::Path::to_path_buf).unwrap_or_else(|| claude_home.clone());
    let extensions = Arc::new(ExtensionsUseCase::new(
        config.clone(),
        Arc::new(ClaudeSkillAdapter::new(claude_home.clone())),
        Arc::new(ClaudePluginAdapter::new(claude_home.clone())),
        Arc::new(ClaudeMcpAdapter::new(claude_json)),
        home,
    ));
```

then `SessionRegistry::new(factory, sessions.clone(), extensions.clone())` and `WorkspaceServices { registry, projects, sessions, settings, extensions }`.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A crates
git commit -m "feat: resolve the enabled extensions when a session's sidecar starts

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Rows, switch, and settings state (tabs' logic and the small items)

**Files:**
- Create: `crates/forge-entities/src/extension_rows.rs`
- Modify: `crates/forge-entities/src/lib.rs`
- Modify: `crates/forge-shared/src/components.rs`, `crates/forge-shared/src/theme.css`
- Create: `crates/forge-features/src/section_status.rs`
- Modify: `crates/forge-features/src/{lib,shortcut,workspace,settings}.rs`
- Modify: `crates/forge-pages/src/workspace_page.rs`

**Interfaces:**
- Consumes: `ExtensionsUseCase` (Task 5) through `WorkspaceServices.extensions` (Task 6); `PluginDefinition::key`.
- Produces:
  - `forge_entities::ExtensionRow { key: String, title: String, detail: String, enabled: bool }`; `skill_rows(&[SkillDefinition], &[String]) -> Vec<ExtensionRow>`, `plugin_rows(&[PluginDefinition], &[String])`, `mcp_rows(&[McpServerDefinition], &[String])`, `mcp_summary(&serde_json::Value) -> String`, `project_label(Option<&Path>) -> String`
  - `forge_shared::Switch { on: bool, disabled: bool = false, onchange: EventHandler<bool> }`
  - `forge_features::shortcut_allowed(Shortcut, overlay_open: bool) -> bool`; `Workspace::on_key(&self, event, overlay_open: bool) -> bool`; `Workspace::context_project(&self) -> Option<PathBuf>`
  - `forge_features::ExtensionKind { Skill, Plugin, McpServer }`; `SettingsSection::{Skills, Plugins, McpServers}`
  - `Settings::skill_tab() / plugin_tab() / mcp_tab() -> Result<Vec<ExtensionRow>, String>`, `Settings::project_label() -> String`, `Settings::set_project(Option<PathBuf>)`, `Settings::toggle(kind: ExtensionKind, key: String)`
  - `section_status::status_for(slot: &Option<(SettingsSection, SettingsStatus)>, current: SettingsSection) -> Option<SettingsStatus>`

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-entities/src/extension_rows.rs` with the tests first (implementation in Step 3 goes above them):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn skill(name: &str, description: Option<&str>) -> SkillDefinition {
        SkillDefinition {
            name: name.to_string(),
            path: PathBuf::from(format!("/skills/{name}")),
            description: description.map(str::to_string),
        }
    }

    fn plugin(name: &str, marketplace: Option<&str>, path: &str) -> PluginDefinition {
        PluginDefinition {
            name: name.to_string(),
            marketplace: marketplace.map(str::to_string),
            path: PathBuf::from(path),
        }
    }

    fn server(name: &str, config: serde_json::Value) -> McpServerDefinition {
        McpServerDefinition { name: name.to_string(), config }
    }

    #[test]
    fn a_skill_row_shows_its_description_and_whether_it_is_enabled() {
        let rows = skill_rows(
            &[skill("pdf", Some("Read PDFs")), skill("docx", None)],
            &["pdf".to_string()],
        );
        assert_eq!(
            rows,
            vec![
                ExtensionRow { key: "pdf".into(), title: "pdf".into(), detail: "Read PDFs".into(), enabled: true },
                ExtensionRow { key: "docx".into(), title: "docx".into(), detail: String::new(), enabled: false },
            ]
        );
    }

    #[test]
    fn two_skills_with_one_name_make_one_row() {
        let rows = skill_rows(&[skill("pdf", Some("user")), skill("pdf", Some("project"))], &[]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].detail, "user", "the first one wins");
    }

    #[test]
    fn a_plugin_row_is_keyed_by_name_at_marketplace_and_shows_the_marketplace() {
        let rows = plugin_rows(
            &[plugin("hud", Some("official"), "/a"), plugin("hud", Some("official"), "/b"), plugin("solo", None, "/c")],
            &["hud@official".to_string()],
        );
        assert_eq!(rows.len(), 2, "the second install of hud is the same plugin");
        assert_eq!(rows[0].key, "hud@official");
        assert_eq!(rows[0].title, "hud");
        assert_eq!(rows[0].detail, "official");
        assert!(rows[0].enabled);
        assert_eq!(rows[1].key, "solo");
        assert_eq!(rows[1].detail, "");
        assert!(!rows[1].enabled);
    }

    #[test]
    fn an_mcp_row_summarises_a_command_or_a_url() {
        let rows = mcp_rows(
            &[
                server("files", serde_json::json!({"command": "npx", "args": ["-y", "files-mcp"]})),
                server("web", serde_json::json!({"type": "http", "url": "https://example.test/mcp"})),
            ],
            &["web".to_string()],
        );
        assert_eq!(rows[0].detail, "npx -y files-mcp");
        assert!(!rows[0].enabled);
        assert_eq!(rows[1].detail, "https://example.test/mcp");
        assert!(rows[1].enabled);
    }

    #[test]
    fn an_mcp_server_with_neither_command_nor_url_still_gets_a_row() {
        let rows = mcp_rows(&[server("odd", serde_json::json!({"type": "sse"}))], &[]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, "odd");
        assert_eq!(rows[0].detail, "");
    }

    #[test]
    fn the_project_label_is_the_folder_name_or_the_global_wording() {
        assert_eq!(project_label(Some(Path::new("/work/forge"))), "forge");
        assert_eq!(project_label(None), "your global definitions");
    }
}
```

In `crates/forge-features/src/shortcut.rs` tests add:

```rust
    #[test]
    fn while_the_overlay_is_open_only_opening_settings_is_allowed() {
        for shortcut in [Shortcut::NewSession, Shortcut::NextSession, Shortcut::PreviousSession, Shortcut::CloseSession] {
            assert!(shortcut_allowed(shortcut, false), "{shortcut:?} with the overlay closed");
            assert!(!shortcut_allowed(shortcut, true), "{shortcut:?} behind the overlay");
        }
        assert!(shortcut_allowed(Shortcut::OpenSettings, true));
        assert!(shortcut_allowed(Shortcut::OpenSettings, false));
    }
```

Create `crates/forge-features/src/section_status.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_shows_only_on_the_section_it_belongs_to() {
        let slot = Some((SettingsSection::Model, SettingsStatus::Saved));
        assert_eq!(status_for(&slot, SettingsSection::Model), Some(SettingsStatus::Saved));
        assert_eq!(status_for(&slot, SettingsSection::Connection), None);
        assert_eq!(status_for(&slot, SettingsSection::Skills), None);
    }

    #[test]
    fn no_status_shows_nothing() {
        assert_eq!(status_for(&None, SettingsSection::Model), None);
    }

    #[test]
    fn an_error_is_kept_with_its_message() {
        let slot = Some((SettingsSection::Plugins, SettingsStatus::Error("boom".to_string())));
        assert_eq!(status_for(&slot, SettingsSection::Plugins), Some(SettingsStatus::Error("boom".to_string())));
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

Register the new modules (`mod extension_rows;` + exports in entities `lib.rs`; `mod section_status;` in features `lib.rs`), then:

Run: `cargo test -p forge-entities -p forge-features`
Expected: compile errors — `skill_rows`, `shortcut_allowed`, `status_for`, `SettingsSection::Skills` not found.

- [ ] **Step 3: Implement**

`crates/forge-entities/src/extension_rows.rs` (above the tests):

```rust
use std::collections::HashSet;
use std::path::Path;

use forge_domain::{McpServerDefinition, PluginDefinition, SkillDefinition};
use serde_json::Value;

/// One line of a Skills / Plugins / MCP servers tab. `key` is what the `enabled_*` list stores.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionRow {
    pub key: String,
    pub title: String,
    pub detail: String,
    pub enabled: bool,
}

/// One row per skill name (a user and a project skill with one name are one entry, as they are for
/// the SDK); the first definition's description is shown.
pub fn skill_rows(definitions: &[SkillDefinition], enabled: &[String]) -> Vec<ExtensionRow> {
    let mut seen = HashSet::new();
    definitions
        .iter()
        .filter(|skill| seen.insert(skill.name.clone()))
        .map(|skill| ExtensionRow {
            key: skill.name.clone(),
            title: skill.name.clone(),
            detail: skill.description.clone().unwrap_or_default(),
            enabled: enabled.contains(&skill.name),
        })
        .collect()
}

/// One row per `name@marketplace` (a plugin installed under two scopes is one plugin).
pub fn plugin_rows(definitions: &[PluginDefinition], enabled: &[String]) -> Vec<ExtensionRow> {
    let mut seen = HashSet::new();
    definitions
        .iter()
        .filter(|plugin| seen.insert(plugin.key()))
        .map(|plugin| ExtensionRow {
            key: plugin.key(),
            title: plugin.name.clone(),
            detail: plugin.marketplace.clone().unwrap_or_default(),
            enabled: enabled.contains(&plugin.key()),
        })
        .collect()
}

pub fn mcp_rows(definitions: &[McpServerDefinition], enabled: &[String]) -> Vec<ExtensionRow> {
    let mut seen = HashSet::new();
    definitions
        .iter()
        .filter(|server| seen.insert(server.name.clone()))
        .map(|server| ExtensionRow {
            key: server.name.clone(),
            title: server.name.clone(),
            detail: mcp_summary(&server.config),
            enabled: enabled.contains(&server.name),
        })
        .collect()
}

/// `command arg…` for a local server, the URL for a remote one, empty when the config has neither
/// (the config is deliberately opaque, spec §12.2).
pub fn mcp_summary(config: &Value) -> String {
    if let Some(command) = config.get("command").and_then(Value::as_str) {
        let args = config
            .get("args")
            .and_then(Value::as_array)
            .map(|args| args.iter().filter_map(Value::as_str).collect::<Vec<_>>())
            .unwrap_or_default();
        return std::iter::once(command).chain(args).collect::<Vec<_>>().join(" ");
    }
    config.get("url").and_then(Value::as_str).unwrap_or_default().to_string()
}

/// What the tabs name as the scope they are showing.
pub fn project_label(project: Option<&Path>) -> String {
    match project.and_then(Path::file_name) {
        Some(name) => name.to_string_lossy().to_string(),
        None => "your global definitions".to_string(),
    }
}
```

`crates/forge-entities/src/lib.rs`: add `mod extension_rows;` and `pub use extension_rows::{mcp_rows, mcp_summary, plugin_rows, project_label, skill_rows, ExtensionRow};`.

`crates/forge-shared/src/components.rs`: add

```rust
/// An on/off switch. Clicking asks for the opposite of the current state via `onchange`.
#[component]
pub fn Switch(on: bool, #[props(default)] disabled: bool, onchange: EventHandler<bool>) -> Element {
    rsx! {
        button {
            class: if on { "switch on" } else { "switch" },
            role: "switch",
            "aria-checked": "{on}",
            disabled,
            onclick: move |_| onchange.call(!on),
            span { class: "switch-knob" }
        }
    }
}
```

and append to `theme.css` (after the `.setting-row` rules):

```css
.switch { flex: none; width: 38px; height: 22px; padding: 2px; border: 1px solid var(--border); border-radius: 999px; background: var(--bg-hover); cursor: pointer; display: inline-flex; align-items: center; }
.switch.on { background: var(--accent); border-color: var(--accent); justify-content: flex-end; }
.switch:disabled { opacity: 0.5; cursor: default; }
.switch-knob { width: 16px; height: 16px; border-radius: 50%; background: var(--bg-card); box-shadow: var(--shadow); }
```

`crates/forge-features/src/shortcut.rs` (above `#[cfg(test)]`):

```rust
/// Whether `shortcut` may act while the settings overlay is open. Only opening settings does: the
/// session shortcuts would change the session hidden behind the overlay (Plan 6 found ⌘W closing it).
pub fn shortcut_allowed(shortcut: Shortcut, overlay_open: bool) -> bool {
    !overlay_open || shortcut == Shortcut::OpenSettings
}
```

`crates/forge-features/src/lib.rs`: export `shortcut_allowed` next to `shortcut_for`, `ExtensionKind` with the settings exports, and add `mod section_status;`.

`crates/forge-features/src/workspace.rs`: import `shortcut_allowed`; change `on_key` to

```rust
    /// Handles the session shortcuts; returns whether the key press was one of them. While the
    /// settings overlay is open they are swallowed (spec §17.5).
    pub fn on_key(&self, event: &KeyboardEvent, overlay_open: bool) -> bool {
        let Some(shortcut) = shortcut_from_event(event) else {
            return false;
        };
        if !shortcut_allowed(shortcut, overlay_open) {
            event.prevent_default();
            return true;
        }
        match shortcut {
            // Opening settings belongs to `Settings::on_key`; the page offers it the event next.
            Shortcut::OpenSettings => return false,
            Shortcut::NewSession => self.show_new_session(),
            Shortcut::NextSession => self.cycle(true),
            Shortcut::PreviousSession => self.cycle(false),
            Shortcut::CloseSession => self.close_active(),
        }
        event.prevent_default();
        true
    }
```

and add next to `new_folder`:

```rust
    /// The project the settings tabs describe: the active session's, or the one the new-session
    /// panel would start in, or none.
    pub fn context_project(&self) -> Option<PathBuf> {
        match self.active() {
            Active::Live(id) => self.live_view(id).map(|view| view.read().project_path.clone()),
            Active::NewSession => self
                .new_folder()
                .or_else(|| self.new_session_projects().into_iter().next().map(|project| project.path)),
        }
    }
```

`crates/forge-features/src/section_status.rs` (above its tests):

```rust
use crate::settings::{SettingsSection, SettingsStatus};

/// The status of the last save, if it belongs to the section being shown. The slot remembers which
/// section it came from, so a save that finishes after the person switched tab does not appear on
/// the new one (spec §17.5).
pub(crate) fn status_for(
    slot: &Option<(SettingsSection, SettingsStatus)>,
    current: SettingsSection,
) -> Option<SettingsStatus> {
    match slot {
        Some((section, status)) if *section == current => Some(status.clone()),
        _ => None,
    }
}
```

`crates/forge-features/src/settings.rs` — apply these changes:

1. Imports: add `use std::path::PathBuf;`, `use forge_application::{ExtensionsUseCase, SettingsUseCase};`, `use forge_domain::{AppConfig, McpServerDefinition, PluginDefinition, SkillDefinition};`, `use forge_entities::{mcp_rows, plugin_rows, project_label, skill_rows, ExtensionRow};`, `use crate::section_status::status_for;`.
2. Enums:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Connection,
    Model,
    Skills,
    Plugins,
    McpServers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionKind {
    Skill,
    Plugin,
    McpServer,
}
```

3. A private `Catalog` and the new state:

```rust
// What the last listing found; an error is shown in its own tab only.
struct Catalog {
    skills: Result<Vec<SkillDefinition>, String>,
    plugins: Result<Vec<PluginDefinition>, String>,
    mcp: Result<Vec<McpServerDefinition>, String>,
}

impl Default for Catalog {
    fn default() -> Self {
        Self { skills: Ok(Vec::new()), plugins: Ok(Vec::new()), mcp: Ok(Vec::new()) }
    }
}
```

`Settings` changes: `status: Signal<Option<(SettingsSection, SettingsStatus)>>`, plus `extensions: Signal<Arc<ExtensionsUseCase>>`, `project: Signal<Option<PathBuf>>`, `catalog: Signal<Catalog>` (initialise with `Signal::new(services.extensions.clone())`, `Signal::new(None)`, `Signal::new(Catalog::default())`). `Signal<Catalog>` needs no `PartialEq`.

4. Methods:
   - `status()` becomes `status_for(&self.status.read(), self.section())`.
   - `set_section` also calls `self.clear_status()`.
   - `open()` also calls `self.reload_catalog()`.
   - `fail(&self, section: SettingsSection, message: String)` sets `Some((section, SettingsStatus::Error(message)))`; the callers pass `SettingsSection::Connection` (save_connection), `SettingsSection::Model` (choose_model) and `*self.section.peek()` (reload). `after_save` stores `(SettingsSection::Connection, Saved)`, `after_save_model` stores `(SettingsSection::Model, Saved)`.
   - New:

```rust
    /// The project whose skills and MCP servers the tabs list (`None`: global definitions only).
    pub fn set_project(&self, project: Option<PathBuf>) {
        let mut current = self.project;
        if *current.peek() != project {
            current.set(project);
        }
    }

    pub fn project_label(&self) -> String {
        project_label(self.project.read().as_deref())
    }

    pub fn skill_tab(&self) -> Result<Vec<ExtensionRow>, String> {
        let enabled = self.saved.read().enabled_skills.clone();
        self.catalog.read().skills.clone().map(|definitions| skill_rows(&definitions, &enabled))
    }

    pub fn plugin_tab(&self) -> Result<Vec<ExtensionRow>, String> {
        let enabled = self.saved.read().enabled_plugins.clone();
        self.catalog.read().plugins.clone().map(|definitions| plugin_rows(&definitions, &enabled))
    }

    pub fn mcp_tab(&self) -> Result<Vec<ExtensionRow>, String> {
        let enabled = self.saved.read().enabled_mcp_servers.clone();
        self.catalog.read().mcp.clone().map(|definitions| mcp_rows(&definitions, &enabled))
    }

    /// Switches one skill, plugin or MCP server on or off and saves at once. `key` is the row's key.
    pub fn toggle(&self, kind: ExtensionKind, key: String) {
        let extensions = (*self.extensions.peek()).clone();
        let this = *self;
        let section = match kind {
            ExtensionKind::Skill => SettingsSection::Skills,
            ExtensionKind::Plugin => SettingsSection::Plugins,
            ExtensionKind::McpServer => SettingsSection::McpServers,
        };
        // Pinned to the root scope, like the other saves: switching tab must not cancel it.
        dioxus::core::spawn_forever(async move {
            let result = match kind {
                ExtensionKind::Skill => extensions.toggle_skill(&key).await,
                ExtensionKind::Plugin => extensions.toggle_plugin(&key).await,
                ExtensionKind::McpServer => extensions.toggle_mcp_server(&key).await,
            };
            match result {
                Ok(config) => this.after_toggle(section, config),
                Err(error) => this.fail(section, error.to_string()),
            }
        });
    }

    // Lists the definitions for the current project; each kind fails on its own.
    fn reload_catalog(&self) {
        let extensions = (*self.extensions.peek()).clone();
        let project = self.project.peek().clone();
        let mut catalog = self.catalog;
        spawn(async move {
            let scope = project.as_deref();
            catalog.set(Catalog {
                skills: extensions.list_skills(scope).await.map_err(|e| e.to_string()),
                plugins: extensions.list_plugins().await.map_err(|e| e.to_string()),
                mcp: extensions.list_mcp_servers(scope).await.map_err(|e| e.to_string()),
            });
        });
    }

    // After a toggle only the enabled lists changed; leave unsaved Connection edits alone.
    fn after_toggle(&self, section: SettingsSection, config: AppConfig) {
        let mut saved = self.saved;
        let mut status = self.status;
        {
            let mut current = saved.write();
            current.enabled_skills = config.enabled_skills;
            current.enabled_plugins = config.enabled_plugins;
            current.enabled_mcp_servers = config.enabled_mcp_servers;
        }
        status.set(Some((section, SettingsStatus::Saved)));
    }
```

`crates/forge-pages/src/workspace_page.rs`: change the key handler to

```rust
            onkeydown: move |event| {
                if !workspace.on_key(&event, settings.is_open()) {
                    settings.on_key(&event);
                }
            },
```

and, after the focus `use_effect`, add:

```rust
    // The tabs list the definitions of the project the person is looking at.
    use_effect(move || settings.set_project(workspace.context_project()));
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --workspace && cargo build -p forge-app`
Expected: PASS and the app compiles (the widgets do not use the new API yet).

- [ ] **Step 5: Commit**

```bash
git add -A crates
git commit -m "feat: extension rows, switch, per-section status and overlay-aware shortcuts

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: The three tabs in the overlay

**Files:**
- Modify: `crates/forge-widgets/src/settings_overlay.rs`

**Interfaces:**
- Consumes: `Settings::{skill_tab, plugin_tab, mcp_tab, project_label, toggle}`, `SettingsSection::{Skills, Plugins, McpServers}`, `ExtensionKind`, `forge_shared::Switch`, `SettingRow`.
- Produces: the visible Skills / Plugins / MCP servers tabs. Widgets are compile-checked here and driven in Task 9 (spec §16.8 / §15.11: runtime problems appear only in a live window).

- [ ] **Step 1: Add the imports and the navigation items**

In `settings_overlay.rs` change the imports to:

```rust
use dioxus::prelude::*;
use forge_entities::{model_from_value, model_options, model_value};
use forge_features::{use_settings, ExtensionKind, SettingsSection, SettingsStatus};
use forge_shared::{Button, ButtonKind, Select, SettingRow, Switch, TextInput};
```

After the Model nav button add three more, in the same shape:

```rust
                    button {
                        class: if section == SettingsSection::Skills { "settings-nav-item active" } else { "settings-nav-item" },
                        onclick: move |_| settings.set_section(SettingsSection::Skills),
                        "Skills"
                    }
                    button {
                        class: if section == SettingsSection::Plugins { "settings-nav-item active" } else { "settings-nav-item" },
                        onclick: move |_| settings.set_section(SettingsSection::Plugins),
                        "Plugins"
                    }
                    button {
                        class: if section == SettingsSection::McpServers { "settings-nav-item active" } else { "settings-nav-item" },
                        onclick: move |_| settings.set_section(SettingsSection::McpServers),
                        "MCP servers"
                    }
```

and extend the content `match`:

```rust
                        SettingsSection::Skills => rsx! { ExtensionSection { kind: ExtensionKind::Skill } },
                        SettingsSection::Plugins => rsx! { ExtensionSection { kind: ExtensionKind::Plugin } },
                        SettingsSection::McpServers => rsx! { ExtensionSection { kind: ExtensionKind::McpServer } },
```

- [ ] **Step 2: Add the shared tab component**

Append to the file:

```rust
/// One of the Skills / Plugins / MCP servers tabs: a row per definition with a switch that saves at once.
#[component]
fn ExtensionSection(kind: ExtensionKind) -> Element {
    let settings = use_settings();
    let (heading, tab, scoped) = match kind {
        ExtensionKind::Skill => ("Skills", settings.skill_tab(), true),
        ExtensionKind::Plugin => ("Plugins", settings.plugin_tab(), false),
        ExtensionKind::McpServer => ("MCP servers", settings.mcp_tab(), true),
    };
    let body = match tab {
        Err(message) => rsx! { div { class: "status-line error", "{message}" } },
        Ok(rows) if rows.is_empty() => rsx! { div { class: "hint", "Nothing found." } },
        Ok(rows) => rsx! {
            for row in rows {
                SettingRow { key: "{row.key}", title: row.title.clone(), description: row.detail.clone(),
                    Switch {
                        on: row.enabled,
                        onchange: {
                            let key = row.key.clone();
                            move |_| settings.toggle(kind, key.clone())
                        },
                    }
                }
            }
        },
    };
    rsx! {
        h2 { "{heading}" }
        if scoped {
            div { class: "hint", "Showing what is available for {settings.project_label()}." }
        }
        {body}
        div { class: "hint", style: "margin-top: 12px;",
            "Sessions that are already open keep what they started with; a change applies to the next new session."
        }
    }
}
```

- [ ] **Step 3: Compile and run the whole suite**

Run: `cargo build -p forge-app && cargo test --workspace`
Expected: builds; all tests pass. (If `for row in rows` inside a `match` arm's `rsx!` does not compile in Dioxus 0.7.10, build the rows into a `Vec<Element>` with `rows.into_iter().map(|row| rsx! { … }).collect::<Vec<_>>()` and render it with `{ … }` — same output.)

- [ ] **Step 4: Commit**

```bash
git add crates/forge-widgets
git commit -m "feat: Skills, Plugins and MCP servers tabs in the settings overlay

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Walk it in a live window, fix what it finds, record it

**Files:**
- Modify: `docs/superpowers/specs/2026-09-27-forge-design.md` (§17.8)
- Modify: whatever the walk finds

**Interfaces:**
- Consumes: everything above; the real `~/.claude`; a built sidecar.
- Produces: §17.8 "As built" — the record, like §15.11 and §16.9.

- [ ] **Step 1: Build and start the app**

```bash
cd sidecar && npm run build && cd ..
cargo run -p forge-app
```

Expected: the window opens. (Plan 6 drove the window with `cliclick` and System Events keystrokes plus screenshots; do the same or do it by hand.)

- [ ] **Step 2: Walk the checklist (spec §17.6)** and note pass / fail / not checked for each

1. `⌘,` → Skills, Plugins, MCP servers tabs list the real definitions of the active project (or "your global definitions" with none active); an empty kind says "Nothing found.".
2. Switch a skill on; close the overlay, quit, start again → still on (open `config.json` and see `enabled_skills`).
3. New session in a project; ask for the skill by name → it is used (it is offered as `forge-skills:<name>`). Switch it off; a new session → it is not offered. A project-local skill (`<project>/.claude/skills`) behaves the same. While the session is open a `forge-skills-*` directory exists under the system temp dir; after closing the session it is gone and the real skill directory is untouched.
4. Switch a plugin on; a new session offers that plugin's skill.
5. Switch an MCP server on (add one first with `claude mcp add` in a scratch project if none exist) → a new session lists it; off → it does not.
6. An older open session is unchanged by any of the above.
7. With the overlay open: `⌘W`, `⌘N`, `Ctrl+Tab` do nothing to the sessions behind it; `Esc` and `⌘,` still work.
8. Save "Saved." on Connection, switch to Model → no "Saved." there; do a model choice, switch to Skills → none there.
9. Nothing enabled anywhere: a new session starts and answers (no reader called).
10. Break a reader on purpose (for example make `~/.claude/plugins/installed_plugins.json` invalid JSON in a scratch `HOME`, with a plugin enabled) → sending shows the reader's message, the text stays in the box, no process is left running. Restore the file.
11. A skill name that exists both user-level and project-level shows once.

- [ ] **Step 3: Fix what the walk finds**

For each defect: write a failing test where the logic is testable, fix, run `cargo test --workspace`, commit as `fix: …` with the trailer. Runtime-only problems (focus, layout) are fixed and named in §17.8 as Plan 6 did.

- [ ] **Step 4: Write §17.8**

Replace the `*(filled in at the end, like §15.11 and §16.9)*` line with: date, macOS and Dioxus versions, the test counts (`cargo test --workspace`, `npm test`), the checklist result per item above, the runtime fixes, differences from §17 (at least: any change the spike forced on §17.3), and what was **not** checked and stays for a person to walk.

- [ ] **Step 5: Final verification and commit**

Run: `cargo test --workspace && cd sidecar && npm test && npm run typecheck`
Expected: all pass.

```bash
git add docs
git commit -m "docs: record Plan 7 as built

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage (§17):**
- 17.1 decisions (toggles reach sessions; Forge-only isolation; out-of-scope list) → Tasks 2–6; isolation options in Task 4; consequence stated in Global Constraints.
- 17.2 SDK facts → Task 1 verifies, Task 4 uses.
- 17.3 protocol and session start (params fields, `ActiveExtensions`, use-case, unknown name skipped, reader failure fails start, registry passes it, sidecar mapping, empty sent as `[]`/`{}`) → Tasks 2, 3, 4, 5, 6. "Wherever the registry starts a sidecar (new and lazily resumed)": both go through `Actor::start_sidecar`, so both are covered by one change (Task 6).
- 17.4 tabs (three sections, rows with description/`name@marketplace`/command-or-url, toggle saves at once through the existing toggle use-cases, project scope named, "already open" line, per-tab reader error, empty state) → Tasks 7 and 8.
- 17.5 small items (overlay swallows session shortcuts; per-section status) → Task 7.
- 17.6 testing + spike + manual checklist → Tasks 1–9.
- 17.7 / 17.8 → Tasks 1 and 9.

**Placeholder scan:** none of "TBD"/"TODO"/"similar to Task N". One conditional is explicit, not a placeholder: Task 4's `settingSources` value follows Task 1's recorded finding, with the alternative spelled out.

**Type consistency:** `ActiveExtensions { skills, plugin_paths, mcp_servers }` (Task 2) is used unchanged in Tasks 3, 5, 6. `ExtensionsUseCase::resolve_active(&Path)` (Task 5) is what the actor calls (Task 6). `toggle_plugin(key)` takes `PluginDefinition::key()` and `plugin_rows` keys rows with the same function (Task 7). `Workspace::on_key(&event, bool)` changes its only caller in the same task. `SessionRegistry::new` gains its third argument in Task 6 in both call sites (registry test helper, `main.rs`).

**Review Focus coverage:** 1 → Task 2 `select_skips_an_enabled_name_that_has_no_definition`; 2 → Task 3 `a_session_without_extensions_still_sends_empty_lists`, Task 4 `passes empty lists…`; 3 → Task 6 `a_reader_failure_spawns_nothing…`, Task 5 `resolve_active_with_nothing_enabled_reads_no_definitions`; 4 → Task 2 `select_lists_a_duplicated_definition_once`, Task 7 `two_skills_with_one_name_make_one_row`; 5 → Task 7 `an_mcp_server_with_neither_command_nor_url_still_gets_a_row`.
