# Plan 32A: the Code tab of Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A "Code" tab in the Settings overlay with four defaults: the transcript view new sessions open in, the output style, prompt suggestions on/off, and whether bypass-permissions may be chosen.

**Architecture:** Four fields on `AppConfig` (all `#[serde(default)]`). The first two reach the UI through `Settings` (immediate save, like `choose_model`). The other three reach the Claude session through one new `SessionDefaults` value that the session actor reads from the config (in the same read that resolves its extensions) at its first sidecar start and puts on `StartSessionParams`; the sidecar turns it into SDK options (`settings.outputStyle`, `promptSuggestions`, `allowDangerouslySkipPermissions`). The mode picker hides the bypass row when the switch is off.

**Tech Stack:** Rust workspace (Dioxus desktop, tokio, serde), Node 22 sidecar (TypeScript, vitest) over `@anthropic-ai/claude-agent-sdk` 0.3.295.

**Spec:** `docs/superpowers/specs/2026-10-09-code-settings-design.md`

## Global Constraints

- Output style is a default for new sessions only; a running session keeps its style. No per-session override (SES-04 is not built).
- The style list is fixed: none (SDK default), `Concise`, `Explanatory`, `Learning`, `Proactive`. Custom styles on disk are not read.
- Controls save when changed. The existing tabs keep their Save button.
- `allow_bypass_mode` is false by default; `prompt_suggestions` is true by default (today's behaviour); `default_transcript_view` is Normal by default; an old config file must load with these defaults.
- A session already in bypass mode keeps that mode when the switch is turned off.
- Rust: `#[serde(default)]` for every new config field; saves go through `SettingsUseCase` and share its write lock.
- Saves from the UI run under `dioxus::core::spawn_forever` (a plain `spawn` is cancelled when the component unmounts).
- Match the surrounding code: comment density, naming, tests next to the code in `mod tests`.
- The worktree has no `sidecar/node_modules`: run `cd sidecar && npm ci` once before any sidecar step.
- Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Review Focus

1. A resumed or forked session whose saved mode is bypass, started while the switch is off: the start must not be refused by the SDK (the actor allows bypass for a session already in that mode). Task 3.
2. The picker of a session that is in bypass mode while the switch is off: the current row stays visible and selected (it must not fall back to the label "Mode"). Task 5.
3. A config file written by an older Forge (no new fields): loads with Normal, no style, suggestions on, bypass off. Task 1.
4. An unknown value in `output_style` or `default_transcript_view` in the file (a hand edit or a newer Forge): the config must still load, with the default. Task 1.
5. A person picks Normal as a session's view while the default is Thinking: the explicit Normal must stick (today Normal means "no entry"). Task 4.

---

## File Structure

- `crates/forge-domain/src/transcript_view.rs` (new, moved from `forge-entities`): `TranscriptView`, now serde-capable.
- `crates/forge-domain/src/output_style.rs` (new): `OutputStyle`.
- `crates/forge-domain/src/session_defaults.rs` (new): `SessionDefaults` and `SessionDefaults::from_config`.
- `crates/forge-domain/src/config.rs`: four fields.
- `crates/forge-entities/src/transcript_view.rs`: deleted; `lib.rs` re-exports from `forge_domain`.
- `crates/forge-ports/src/sidecar_port.rs`: `StartSessionParams.defaults`.
- `crates/forge-adapter-sidecar/src/protocol.rs`: three wire fields.
- `crates/forge-application/src/use_cases/extensions.rs`, `live_session_actor.rs`: read the defaults with the extensions at the first start.
- `crates/forge-application/src/use_cases/settings.rs`: `save_code_settings`.
- `sidecar/src/protocol.ts`, `sidecar/src/session.ts`, `sidecar/test/session.test.ts`, `sidecar/scripts/check_output_style.mjs` (new).
- `crates/forge-features/src/settings.rs`: `SettingsSection::Code` and the four setters/getters.
- `crates/forge-features/src/transcript_views.rs`, `workspace.rs`: view with a default.
- `crates/forge-features/src/permission_mode.rs`: `mode_rows(include_bypass)`.
- `crates/forge-widgets/src/settings_overlay.rs`, `composer_controls.rs`, `chat_window.rs`: tab, picker, chat header.

---

### Task 1: Config fields and domain types

**Files:**
- Move: `crates/forge-entities/src/transcript_view.rs` → `crates/forge-domain/src/transcript_view.rs`
- Create: `crates/forge-domain/src/output_style.rs`, `crates/forge-domain/src/session_defaults.rs`
- Modify: `crates/forge-domain/src/lib.rs`, `crates/forge-domain/src/config.rs`, `crates/forge-entities/src/lib.rs`, `crates/forge-entities/src/turn.rs:15`, `crates/forge-entities/src/turn_view.rs:9`

**Interfaces:**
- Produces:
  - `forge_domain::TranscriptView` (same variants and methods as today, plus `Serialize, Deserialize` with `rename_all = "snake_case"`); `forge_entities::TranscriptView` stays as a re-export.
  - `forge_domain::OutputStyle { Concise, Explanatory, Learning, Proactive }` with `OutputStyle::ALL: [OutputStyle; 4]`, `name(self) -> &'static str` (`"Concise"`, …), `from_name(&str) -> Option<OutputStyle>`.
  - `AppConfig.default_transcript_view: TranscriptView`, `AppConfig.output_style: Option<OutputStyle>`, `AppConfig.prompt_suggestions: bool`, `AppConfig.allow_bypass_mode: bool`.
  - `forge_domain::SessionDefaults { output_style: Option<OutputStyle>, prompt_suggestions: bool, allow_bypass: bool }` with `Default` (None, true, false) and `SessionDefaults::from_config(&AppConfig) -> SessionDefaults`.

- [ ] **Step 1: Move `TranscriptView` into the domain crate**

```bash
git mv crates/forge-entities/src/transcript_view.rs crates/forge-domain/src/transcript_view.rs
```

In `crates/forge-domain/src/transcript_view.rs` change the derive line to:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptView {
```

In `crates/forge-domain/src/lib.rs` add `mod transcript_view;` with the other `mod` lines and `pub use transcript_view::TranscriptView;` with the other `pub use` lines.

In `crates/forge-entities/src/lib.rs` delete `mod transcript_view;` (line 21) and replace `pub use transcript_view::TranscriptView;` (line 45) with `pub use forge_domain::TranscriptView;`.

In `crates/forge-entities/src/turn.rs:15` and `turn_view.rs:9` replace `use crate::transcript_view::TranscriptView;` with `use forge_domain::TranscriptView;`.

- [ ] **Step 2: Check the move builds**

Run: `cargo test -p forge-domain -p forge-entities`
Expected: PASS (the four `transcript_view` tests now run in `forge-domain`).

- [ ] **Step 3: Write the failing tests**

Create `crates/forge-domain/src/output_style.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Claude Code's own output styles that the desktop app offers besides the default. The SDK reads
/// the name as written here (`settings.outputStyle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStyle {
    Concise,
    Explanatory,
    Learning,
    Proactive,
}

impl OutputStyle {
    pub const ALL: [OutputStyle; 4] = [OutputStyle::Concise, OutputStyle::Explanatory, OutputStyle::Learning, OutputStyle::Proactive];

    /// The name the SDK and the menu use.
    pub fn name(self) -> &'static str {
        match self {
            OutputStyle::Concise => "Concise",
            OutputStyle::Explanatory => "Explanatory",
            OutputStyle::Learning => "Learning",
            OutputStyle::Proactive => "Proactive",
        }
    }

    pub fn from_name(name: &str) -> Option<OutputStyle> {
        OutputStyle::ALL.into_iter().find(|style| style.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_is_found_again_by_its_name() {
        for style in OutputStyle::ALL {
            assert_eq!(OutputStyle::from_name(style.name()), Some(style));
        }
    }

    #[test]
    fn the_default_style_and_unknown_names_are_not_styles() {
        assert_eq!(OutputStyle::from_name("default"), None);
        assert_eq!(OutputStyle::from_name(""), None);
        assert_eq!(OutputStyle::from_name("concise"), None, "names are case-sensitive, as the SDK reads them");
    }
}
```

Create `crates/forge-domain/src/session_defaults.rs`:

```rust
use crate::{AppConfig, OutputStyle};

/// What the Code tab of Settings decides for a session at the moment its sidecar starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionDefaults {
    pub output_style: Option<OutputStyle>,
    pub prompt_suggestions: bool,
    /// Whether the SDK may enter bypass-permissions mode in this session.
    pub allow_bypass: bool,
}

impl Default for SessionDefaults {
    fn default() -> Self {
        Self { output_style: None, prompt_suggestions: true, allow_bypass: false }
    }
}

impl SessionDefaults {
    pub fn from_config(config: &AppConfig) -> Self {
        Self { output_style: config.output_style, prompt_suggestions: config.prompt_suggestions, allow_bypass: config.allow_bypass_mode }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_config_gives_the_default_defaults() {
        assert_eq!(SessionDefaults::from_config(&AppConfig::default()), SessionDefaults::default());
    }

    #[test]
    fn the_config_values_are_carried() {
        let config = AppConfig { output_style: Some(OutputStyle::Learning), prompt_suggestions: false, allow_bypass_mode: true, ..AppConfig::default() };
        assert_eq!(
            SessionDefaults::from_config(&config),
            SessionDefaults { output_style: Some(OutputStyle::Learning), prompt_suggestions: false, allow_bypass: true }
        );
    }
}
```

Register both in `crates/forge-domain/src/lib.rs` (`mod output_style; mod session_defaults;` and `pub use output_style::OutputStyle; pub use session_defaults::SessionDefaults;`).

Append these tests to `mod tests` in `crates/forge-domain/src/config.rs`:

```rust
    #[test]
    fn an_old_config_loads_with_the_code_tab_defaults() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":null,"api_key":null}"#).unwrap();
        assert_eq!(config.default_transcript_view, crate::TranscriptView::Normal);
        assert_eq!(config.output_style, None);
        assert!(config.prompt_suggestions, "suggestions stay on for a config written before the switch existed");
        assert!(!config.allow_bypass_mode);
        assert_eq!(AppConfig::default().prompt_suggestions, true, "the derived Default agrees with the file default");
    }

    #[test]
    fn the_code_tab_values_round_trip() {
        let config = AppConfig {
            default_transcript_view: crate::TranscriptView::Detailed,
            output_style: Some(crate::OutputStyle::Concise),
            prompt_suggestions: false,
            allow_bypass_mode: true,
            ..AppConfig::default()
        };
        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(back, config);
    }

    #[test]
    fn an_unknown_style_or_view_in_the_file_does_not_stop_the_config_loading() {
        let config: AppConfig =
            serde_json::from_str(r#"{"base_url":null,"api_key":null,"output_style":"poetic","default_transcript_view":"loud"}"#).unwrap();
        assert_eq!(config.output_style, None);
        assert_eq!(config.default_transcript_view, crate::TranscriptView::Normal);
    }
```

- [ ] **Step 4: Run the tests to see them fail**

Run: `cargo test -p forge-domain`
Expected: FAIL to compile (`no field default_transcript_view on type AppConfig`).

- [ ] **Step 5: Add the fields**

`AppConfig` derives `Default`, which would make `prompt_suggestions` false, so `Default` must become a manual impl. In `crates/forge-domain/src/config.rs` change the derive of `AppConfig` to `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]`, add the fields after `routine_runs`:

```rust
    /// The transcript view a session opens in until the person picks one for it.
    #[serde(default, deserialize_with = "lenient")]
    pub default_transcript_view: crate::TranscriptView,
    /// The output style new sessions start with; `None` is the SDK's own default.
    #[serde(default, deserialize_with = "lenient")]
    pub output_style: Option<crate::OutputStyle>,
    /// Whether Claude predicts the next prompt after each answer.
    #[serde(default = "default_true")]
    pub prompt_suggestions: bool,
    /// Whether the bypass-permissions mode may be chosen at all.
    #[serde(default)]
    pub allow_bypass_mode: bool,
```

and below the struct:

```rust
fn default_true() -> bool {
    true
}

// A value this Forge does not know (a hand edit, or a newer Forge) reads as the default, so the
// rest of the config still loads.
fn lenient<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            base_url: None,
            api_key: None,
            model: None,
            enabled_skills: Vec::new(),
            enabled_plugins: Vec::new(),
            enabled_mcp_servers: Vec::new(),
            sidebar: SidebarPrefs::default(),
            editor: EditorPrefs::default(),
            archived: Vec::new(),
            routines: Vec::new(),
            routine_runs: Vec::new(),
            default_transcript_view: crate::TranscriptView::default(),
            output_style: None,
            prompt_suggestions: true,
            allow_bypass_mode: false,
        }
    }
}
```

Check that the field list in `Default` matches the struct exactly (read the struct once more; if a field was added since this plan was written, add it).

- [ ] **Step 6: Run the tests to see them pass**

Run: `cargo test -p forge-domain -p forge-entities`
Expected: PASS. Then `cargo build --workspace` must compile (`AppConfig { .. }` literals elsewhere use `..AppConfig::default()`; fix any that do not).

- [ ] **Step 7: Commit**

```bash
git add -A crates/forge-domain crates/forge-entities
git commit -m "feat: AppConfig gets the Code tab fields (default view, output style, prompt suggestions, bypass switch) and TranscriptView moves to forge-domain

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: The sidecar carries the three options

**Files:**
- Modify: `crates/forge-ports/src/sidecar_port.rs` (struct at line 14, test literal at ~159), `crates/forge-adapter-sidecar/src/protocol.rs`, `sidecar/src/protocol.ts`, `sidecar/src/session.ts:334-360`, `sidecar/test/session.test.ts`
- Modify (literals only): every `StartSessionParams {` in the workspace (18 places: `forge-application` `live_session.rs`, `live_session_actor.rs`, `registry_host.rs`, `command_probe.rs`, `use_cases/session_lifecycle.rs`; `forge-adapter-sidecar` `examples/spike_drive_sidecar.rs`, `protocol.rs`)

**Interfaces:**
- Consumes: `SessionDefaults` (Task 1).
- Produces:
  - `StartSessionParams.defaults: SessionDefaults`.
  - Wire (`start_session`): `output_style?: string` (left out when none), `prompt_suggestions: boolean`, `allow_bypass: boolean`.
  - Sidecar: `settings: { outputStyle }` in the query options only when `output_style` is set; `promptSuggestions: command.prompt_suggestions`; `allowDangerouslySkipPermissions: command.allow_bypass`.

- [ ] **Step 1: Write the failing Rust protocol tests**

In `crates/forge-adapter-sidecar/src/protocol.rs`, `mod tests`, after `params_with`, add:

```rust
    #[test]
    fn start_session_carries_the_defaults_and_leaves_the_style_out_when_there_is_none() {
        let json = serde_json::to_value(SidecarCommand::start_session("1".to_string(), params_with(Vec::new()))).unwrap();
        assert!(json.get("output_style").is_none());
        assert_eq!(json["prompt_suggestions"], serde_json::json!(true));
        assert_eq!(json["allow_bypass"], serde_json::json!(false));
    }

    #[test]
    fn start_session_carries_the_style_name_and_the_switches() {
        let mut params = params_with(Vec::new());
        params.defaults = forge_domain::SessionDefaults {
            output_style: Some(forge_domain::OutputStyle::Concise),
            prompt_suggestions: false,
            allow_bypass: true,
        };
        let json = serde_json::to_value(SidecarCommand::start_session("1".to_string(), params)).unwrap();
        assert_eq!(json["output_style"], serde_json::json!("Concise"));
        assert_eq!(json["prompt_suggestions"], serde_json::json!(false));
        assert_eq!(json["allow_bypass"], serde_json::json!(true));
    }
```



- [ ] **Step 2: Run to see failure**

Run: `cargo test -p forge-adapter-sidecar protocol`
Expected: FAIL to compile (`no field defaults`).

- [ ] **Step 3: Add the field and the wire fields**

`crates/forge-ports/src/sidecar_port.rs`: add `SessionDefaults` to the `forge_domain` import and, to `StartSessionParams`:

```rust
    /// What the Code tab of Settings decides for this start.
    pub defaults: SessionDefaults,
```

`crates/forge-adapter-sidecar/src/protocol.rs`: in `SidecarCommand::StartSession` add

```rust
        #[serde(skip_serializing_if = "Option::is_none")]
        output_style: Option<String>,
        prompt_suggestions: bool,
        allow_bypass: bool,
```

and in the constructor (line ~118, after `allowed_tools: params.allowed_tools,`):

```rust
            output_style: params.defaults.output_style.map(|style| style.name().to_string()),
            prompt_suggestions: params.defaults.prompt_suggestions,
            allow_bypass: params.defaults.allow_bypass,
```

Then add `defaults: SessionDefaults::default(),` to every `StartSessionParams { .. }` literal that the compiler lists (`cargo build --workspace --all-targets` names each; import `forge_domain::SessionDefaults` in those files). In `live_session_actor.rs:244` write `defaults: SessionDefaults::default(),` for now; Task 3 replaces it.

- [ ] **Step 4: Run to see the Rust side pass**

Run: `cargo test -p forge-adapter-sidecar -p forge-ports && cargo build --workspace --all-targets`
Expected: PASS.

- [ ] **Step 5: Write the failing sidecar tests**

`cd sidecar && npm ci` once. In `sidecar/test/session.test.ts` change `startCommand` to carry the new fields:

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
  prompt_suggestions: true,
  allow_bypass: true,
};
```

(`allow_bypass: true` keeps the existing test at line ~1155 valid.) Add a new `describe` after the "allows bypassPermissions" test's describe:

```ts
  describe("the Code tab defaults", () => {
    function capture(): { options: () => any; queryFn: QueryFn } {
      let captured: any;
      const queryFn: QueryFn = (args) => {
        captured = args.options;
        return toQueryHandle((async function* () {})());
      };
      return { options: () => captured, queryFn };
    }

    it("passes the output style as an inline setting, and no setting when there is none", async () => {
      const styled = capture();
      await new SidecarSession(styled.queryFn, () => {}).handle({ ...startCommand, output_style: "Concise" });
      expect(styled.options().settings).toEqual({ outputStyle: "Concise" });

      const plain = capture();
      await new SidecarSession(plain.queryFn, () => {}).handle(startCommand);
      expect(plain.options().settings).toBeUndefined();
    });

    it("follows the command for prompt suggestions", async () => {
      const off = capture();
      await new SidecarSession(off.queryFn, () => {}).handle({ ...startCommand, prompt_suggestions: false });
      expect(off.options().promptSuggestions).toBe(false);
    });

    it("lets the SDK enter bypass only when the command allows it", async () => {
      const refused = capture();
      await new SidecarSession(refused.queryFn, () => {}).handle({ ...startCommand, allow_bypass: false });
      expect(refused.options().allowDangerouslySkipPermissions).toBe(false);

      const allowed = capture();
      await new SidecarSession(allowed.queryFn, () => {}).handle({ ...startCommand, allow_bypass: true });
      expect(allowed.options().allowDangerouslySkipPermissions).toBe(true);
    });
  });
```

- [ ] **Step 6: Run to see failure**

Run: `cd sidecar && npx vitest run test/session.test.ts`
Expected: FAIL (type errors on the new fields, or `settings` undefined where "Concise" is expected).

- [ ] **Step 7: Implement in the sidecar**

`sidecar/src/protocol.ts`, in the `start_session` variant add:

```ts
      output_style?: string;
      prompt_suggestions: boolean;
      allow_bypass: boolean;
```

`sidecar/src/session.ts`: replace the `allowDangerouslySkipPermissions: true,` line and its comment with

```ts
          // The SDK refuses bypassPermissions later unless this is set now. It only lets the mode be
          // chosen; Forge enters it only when the person confirms, and only when Settings allows it.
          allowDangerouslySkipPermissions: command.allow_bypass,
```

replace `promptSuggestions: true,` with `promptSuggestions: command.prompt_suggestions,` (keep the comment line above it, adjusting "and predicts the next prompt" to "and predicts the next prompt unless Settings turns it off"), and add inside `options` (after `plugins,`):

```ts
          // The output style of a new session; the SDK's default style when none is set.
          ...(command.output_style ? { settings: { outputStyle: command.output_style } } : {}),
```

- [ ] **Step 8: Run to see pass**

Run: `cd sidecar && npx vitest run && npm run typecheck`
Expected: PASS (all sidecar tests, including the old ones at lines ~912 and ~1155).

- [ ] **Step 9: Commit**

```bash
git add -A crates sidecar
git commit -m "feat: the start command carries output style, prompt suggestions and the bypass switch; the sidecar turns them into SDK options

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The session reads the defaults when its sidecar starts

**Files:**
- Modify: `crates/forge-application/src/use_cases/extensions.rs` (`resolve_active`, line 69), `crates/forge-application/src/live_session_actor.rs` (struct, `new`, `start_sidecar`), `crates/forge-application/src/test_support.rs`, tests in `crates/forge-application/src/live_session.rs`

**Interfaces:**
- Consumes: `SessionDefaults::from_config` (Task 1), `StartSessionParams.defaults` (Task 2).
- Produces:
  - `ExtensionsUseCase::resolve_start(&self, project: &Path) -> Result<(ActiveExtensions, SessionDefaults), ExtensionsError>`: one read of the config decides both. `resolve_active` stays and returns the first half.
  - A session's first successful start stores both; later restarts of the same session reuse them (config is read once per open session).
  - Bypass is allowed in the start of a session whose own mode is `BypassPermissions`.
  - Test helper `extensions_changing_config(first: AppConfig, later: AppConfig) -> Arc<ExtensionsUseCase>`: the first `load` returns `first`, every later one `later`.

- [ ] **Step 1: Write the failing tests**

In `crates/forge-application/src/test_support.rs`, after `extensions_with`, add:

```rust
/// The first read of the config gives `first`, every later read `later`: a config edited while a
/// session is open.
pub(crate) fn extensions_changing_config(first: AppConfig, later: AppConfig) -> Arc<ExtensionsUseCase> {
    let reads = Arc::new(AtomicUsize::new(0));
    let mut config_port = MockConfigRepositoryPort::new();
    config_port.expect_load().returning(move || {
        Ok(if reads.fetch_add(1, Ordering::SeqCst) == 0 { first.clone() } else { later.clone() })
    });
    Arc::new(ExtensionsUseCase::new(
        Arc::new(config_port),
        Arc::new(MockSkillRepositoryPort::new()),
        Arc::new(MockPluginRepositoryPort::new()),
        Arc::new(MockMcpRegistryPort::new()),
        PathBuf::from("/home/me"),
    ))
}
```

In `live_session.rs` tests: add `defaults: SessionDefaults::default(),` to `start_params` (import `forge_domain::{OutputStyle, SessionDefaults}` and `extensions_changing_config` in the `test_support` import list), then add:

```rust
    fn code_tab_config() -> AppConfig {
        AppConfig {
            output_style: Some(OutputStyle::Concise),
            prompt_suggestions: false,
            allow_bypass_mode: true,
            ..AppConfig::default()
        }
    }

    fn started_defaults(calls: Vec<Call>) -> SessionDefaults {
        match calls.into_iter().next() {
            Some(Call::Start(params)) => params.defaults,
            other => panic!("expected a start, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn the_first_start_carries_the_code_tab_defaults_from_the_config() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let mut init = new_init(factory);
        init.extensions = extensions_with(code_tab_config(), Vec::new());
        let handle = spawn_live_session(init);
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert_eq!(
            started_defaults(control.calls()),
            SessionDefaults { output_style: Some(OutputStyle::Concise), prompt_suggestions: false, allow_bypass: true }
        );
    }

    #[tokio::test]
    async fn a_restarted_session_keeps_the_defaults_it_first_started_with() {
        let (first, first_control) = fake_sidecar();
        let (second, second_control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(first), Ok(second)]);
        let mut init = new_init(factory);
        // The person turns everything back to the stock values while the session is open.
        init.extensions = extensions_changing_config(code_tab_config(), AppConfig::default());
        let handle = spawn_live_session(init);
        let mut view = handle.subscribe();
        handle.send("one".to_string());
        first_control.push(SidecarEvent::Error { message: "boom".to_string() });
        wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        handle.send("two".to_string());
        wait_for(&mut view, "the second turn", |v| v.phase == Phase::Running).await;

        let first_defaults = started_defaults(first_control.calls());
        assert_eq!(first_defaults.output_style, Some(OutputStyle::Concise));
        assert_eq!(started_defaults(second_control.calls()), first_defaults);
    }

    #[tokio::test]
    async fn a_session_already_in_bypass_mode_may_start_with_the_switch_off() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let mut init = resumed_init(factory);
        init.session.permission_mode = PermissionMode::BypassPermissions;
        // The default config: the switch is off.
        let handle = spawn_live_session(init);
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert!(started_defaults(control.calls()).allow_bypass, "the SDK refuses the mode unless it is allowed up front");
    }

    #[tokio::test]
    async fn a_session_in_another_mode_does_not_get_bypass_when_the_switch_is_off() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert!(!started_defaults(control.calls()).allow_bypass);
    }
```

If `AppConfig`, `PermissionMode` or the atomics are not yet imported where these are used, add them to the existing `use` lines of the module.

In `use_cases/extensions.rs` `mod tests`, add:

```rust
    #[tokio::test]
    async fn resolve_start_gives_the_extensions_and_the_defaults_from_one_read_of_the_config() {
        let config = AppConfig { output_style: Some(forge_domain::OutputStyle::Proactive), allow_bypass_mode: true, ..AppConfig::default() };
        let mut config_port = MockConfigRepositoryPort::new();
        config_port.expect_load().times(1).returning(move || Ok(config.clone()));

        let (active, defaults) = ExtensionsUseCase::new(
            Arc::new(config_port),
            Arc::new(MockSkillRepositoryPort::new()),
            Arc::new(MockPluginRepositoryPort::new()),
            Arc::new(MockMcpRegistryPort::new()),
            PathBuf::from("/home/me"),
        )
        .resolve_start(Path::new("/work/a"))
        .await
        .unwrap();

        assert!(active.skills.is_empty());
        assert_eq!(defaults.output_style, Some(forge_domain::OutputStyle::Proactive));
        assert!(defaults.allow_bypass);
    }
```

(Copy the `use_case(...)` helper call shape from the neighbouring tests if it is shorter than building the use case by hand.)

- [ ] **Step 2: Run to see failure**

Run: `cargo test -p forge-application`
Expected: FAIL to compile (`resolve_start` missing).

- [ ] **Step 3: Implement**

`extensions.rs`: rename the body of `resolve_active` into

```rust
    /// What a session starts with, from one read of the config: the enabled extensions and the
    /// Code tab defaults. (The extensions part is described on `resolve_active`.)
    pub async fn resolve_start(&self, project: &Path) -> Result<(ActiveExtensions, SessionDefaults), ExtensionsError> {
        let config = self.config.load().await?;
        let skills = /* unchanged */;
        let plugins = /* unchanged */;
        let servers = /* unchanged */;
        Ok((ActiveExtensions::select(&config, &skills, &plugins, &servers), SessionDefaults::from_config(&config)))
    }

    pub async fn resolve_active(&self, project: &Path) -> Result<ActiveExtensions, ExtensionsError> {
        Ok(self.resolve_start(project).await?.0)
    }
```

(move the existing doc comment of `resolve_active` to stay on `resolve_active`; import `forge_domain::SessionDefaults`.)

`live_session_actor.rs`: add the field next to `active`

```rust
    /// What Settings decided at the first successful start; kept across restarts like `active`
    /// (a change in Settings applies to the next new session).
    defaults: Option<SessionDefaults>,
```

with `defaults: None,` in `new`, import `forge_domain::SessionDefaults`, and in `start_sidecar` replace the `let extensions = match &self.active { … };` with

```rust
        let (extensions, configured) = match (&self.active, self.defaults) {
            (Some(active), Some(defaults)) => (active.clone(), defaults),
            _ => self.extensions.resolve_start(&self.view.project_path).await.map_err(|e| e.to_string())?,
        };
        // A session already in bypass mode (resumed, or switched before the switch went off) must
        // still start: the SDK refuses that mode unless it is allowed up front.
        let mut defaults = configured;
        defaults.allow_bypass |= self.view.session.permission_mode == PermissionMode::BypassPermissions;
```

pass `defaults,` in the `StartSessionParams` literal (replacing the `SessionDefaults::default()` placeholder from Task 2), and after `self.active = Some(extensions);` add `self.defaults = Some(configured);` (the stored value is the configured one, so a restart applies the bypass rule against the mode at that time).

- [ ] **Step 4: Run to see pass**

Run: `cargo test -p forge-application`
Expected: PASS, including the existing `a_restarted_session_keeps_the_extensions_it_first_started_with` (`loads == 1`).

- [ ] **Step 5: Commit**

```bash
git add -A crates/forge-application
git commit -m "feat: a session reads the Code tab defaults with its extensions at the first sidecar start and keeps them across restarts

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Saving the Code settings and the default transcript view

**Files:**
- Modify: `crates/forge-application/src/use_cases/settings.rs`, `crates/forge-features/src/settings.rs`, `crates/forge-features/src/transcript_views.rs`, `crates/forge-features/src/workspace.rs:510-512`, `crates/forge-features/src/section_status.rs` (tests only if needed), `crates/forge-widgets/src/chat_window.rs:55`

**Interfaces:**
- Consumes: the `AppConfig` fields (Task 1).
- Produces:
  - `SettingsUseCase::save_code_settings(&self, change: impl FnOnce(&mut AppConfig)) -> Result<AppConfig, ConfigError>` (loads, applies `change`, saves under the lock; a config that cannot be read is not replaced, like `save_sidebar`).
  - `SettingsSection::Code`.
  - `Settings::default_transcript_view() -> TranscriptView`, `Settings::output_style() -> Option<OutputStyle>`, `Settings::prompt_suggestions() -> bool`, `Settings::allow_bypass_mode() -> bool`, and setters `set_default_transcript_view(TranscriptView)`, `set_output_style(Option<OutputStyle>)`, `set_prompt_suggestions(bool)`, `set_allow_bypass_mode(bool)`.
  - `TranscriptViews::get(id, default) -> TranscriptView`; `set` always stores. `Workspace::transcript_view(id, default)`.

- [ ] **Step 1: Write the failing tests**

In `use_cases/settings.rs` `mod tests` (copy the mock style of the neighbouring `save_*` tests):

```rust
    #[tokio::test]
    async fn save_code_settings_changes_only_what_the_closure_changes_and_saves_once() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().returning(|| Ok(AppConfig { model: Some("m".to_string()), ..AppConfig::default() }));
        mock.expect_save()
            .withf(|config| config.allow_bypass_mode && config.model.as_deref() == Some("m"))
            .times(1)
            .returning(|_| Ok(()));

        let saved = SettingsUseCase::new(Arc::new(mock)).save_code_settings(|config| config.allow_bypass_mode = true).await.unwrap();
        assert!(saved.allow_bypass_mode);
    }

    #[tokio::test]
    async fn save_code_settings_never_replaces_a_config_that_cannot_be_read() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().returning(|| Err(ConfigError::ParseFailed("bad".to_string())));
        // No expect_save: a save would panic.
        assert!(SettingsUseCase::new(Arc::new(mock)).save_code_settings(|config| config.prompt_suggestions = false).await.is_err());
    }
```


In `transcript_views.rs` replace the tests with the default-aware versions and add the focus case:

```rust
    #[test]
    fn a_session_without_a_choice_follows_the_default() {
        let views = TranscriptViews::default();
        assert_eq!(views.get(LiveSessionId(1), TranscriptView::Normal), TranscriptView::Normal);
        assert_eq!(views.get(LiveSessionId(1), TranscriptView::Detailed), TranscriptView::Detailed);
    }

    #[test]
    fn each_session_keeps_its_own_mode() {
        let mut views = TranscriptViews::default();
        views.set(LiveSessionId(1), TranscriptView::Thinking);
        views.set(LiveSessionId(2), TranscriptView::Detailed);

        assert_eq!(views.get(LiveSessionId(1), TranscriptView::Normal), TranscriptView::Thinking);
        assert_eq!(views.get(LiveSessionId(2), TranscriptView::Normal), TranscriptView::Detailed);
        assert_eq!(views.get(LiveSessionId(3), TranscriptView::Normal), TranscriptView::Normal);
    }

    #[test]
    fn a_chosen_normal_sticks_when_the_default_is_something_else() {
        let mut views = TranscriptViews::default();
        views.set(LiveSessionId(1), TranscriptView::Normal);
        assert_eq!(views.get(LiveSessionId(1), TranscriptView::Thinking), TranscriptView::Normal);
    }

    #[test]
    fn a_closed_session_goes_back_to_the_default() {
        let mut views = TranscriptViews::default();
        views.set(LiveSessionId(1), TranscriptView::Detailed);
        views.forget(LiveSessionId(1));
        assert_eq!(views.get(LiveSessionId(1), TranscriptView::Thinking), TranscriptView::Thinking);
    }
```

(Keep any other existing test in the file, converting `get(id)` calls to `get(id, TranscriptView::Normal)`.)

- [ ] **Step 2: Run to see failure**

Run: `cargo test -p forge-application settings && cargo test -p forge-features transcript_views`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the use case and `TranscriptViews`**

`use_cases/settings.rs`, after `save_editor`:

```rust
    /// Saves the Code tab's switches: loads the config, lets `change` edit it, writes it back. Like
    /// `save_sidebar`, a config that cannot be read is never replaced, and it shares the write lock.
    pub async fn save_code_settings(&self, change: impl FnOnce(&mut AppConfig)) -> Result<AppConfig, ConfigError> {
        let _saving = self.write_lock.hold().await;
        let mut config = self.config.load().await?;
        change(&mut config);
        self.config.save(&config).await?;
        Ok(config)
    }
```

`transcript_views.rs`:

```rust
    /// The session's own choice, else `default` (the view Settings says new sessions open in).
    pub fn get(&self, id: LiveSessionId, default: TranscriptView) -> TranscriptView {
        self.modes.get(&id).copied().unwrap_or(default)
    }

    /// Always stores: with a default that is not Normal, a chosen Normal must stay a choice.
    pub fn set(&mut self, id: LiveSessionId, view: TranscriptView) {
        self.modes.insert(id, view);
    }
```

(and update the doc comment: "…a session that is opened again starts in the default view").

`workspace.rs:510`:

```rust
    /// How much of the open session the chat shows: the person's choice for it, else `default`.
    pub fn transcript_view(&self, id: LiveSessionId, default: TranscriptView) -> TranscriptView {
        self.transcript_views.read().get(id, default)
    }
```

- [ ] **Step 4: Run to see pass**

Run: `cargo test -p forge-application settings && cargo test -p forge-features transcript_views`
Expected: PASS for these (the rest of `forge-features` and `forge-widgets` will not compile until the caller is updated in Step 6).

- [ ] **Step 5: Write the failing `Settings` test**

`Settings` is a Dioxus signal holder, so the pure part is tested instead: add to `settings.rs` a free function and test it.

```rust
/// What a Code tab control does to the stored config. One function per control keeps the setters
/// below one line each and the effect testable without a runtime.
pub(crate) fn apply_code_change(config: &mut AppConfig, change: CodeChange) {
    match change {
        CodeChange::DefaultView(view) => config.default_transcript_view = view,
        CodeChange::OutputStyle(style) => config.output_style = style,
        CodeChange::PromptSuggestions(on) => config.prompt_suggestions = on,
        CodeChange::AllowBypass(on) => config.allow_bypass_mode = on,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CodeChange {
    DefaultView(TranscriptView),
    OutputStyle(Option<OutputStyle>),
    PromptSuggestions(bool),
    AllowBypass(bool),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_control_changes_its_own_field_only() {
        let mut config = AppConfig::default();
        apply_code_change(&mut config, CodeChange::DefaultView(TranscriptView::Thinking));
        apply_code_change(&mut config, CodeChange::OutputStyle(Some(OutputStyle::Learning)));
        apply_code_change(&mut config, CodeChange::PromptSuggestions(false));
        apply_code_change(&mut config, CodeChange::AllowBypass(true));

        assert_eq!(config.default_transcript_view, TranscriptView::Thinking);
        assert_eq!(config.output_style, Some(OutputStyle::Learning));
        assert!(!config.prompt_suggestions);
        assert!(config.allow_bypass_mode);
        assert_eq!(config.model, None, "nothing else is touched");
    }

    #[test]
    fn choosing_the_default_style_clears_it() {
        let mut config = AppConfig { output_style: Some(OutputStyle::Concise), ..AppConfig::default() };
        apply_code_change(&mut config, CodeChange::OutputStyle(None));
        assert_eq!(config.output_style, None);
    }
}
```

(`settings.rs` has no `mod tests` today; add it at the bottom. Add `use forge_domain::{OutputStyle, TranscriptView};` to the imports.)

- [ ] **Step 6: Implement `Settings`**

Add `Code` to `SettingsSection`. Add the getters and setters in `impl Settings` (after `model`):

```rust
    pub fn default_transcript_view(&self) -> TranscriptView {
        self.saved.read().default_transcript_view
    }

    pub fn output_style(&self) -> Option<OutputStyle> {
        self.saved.read().output_style
    }

    pub fn prompt_suggestions(&self) -> bool {
        self.saved.read().prompt_suggestions
    }

    pub fn allow_bypass_mode(&self) -> bool {
        self.saved.read().allow_bypass_mode
    }

    pub fn set_default_transcript_view(&self, view: TranscriptView) {
        self.change_code(CodeChange::DefaultView(view));
    }

    pub fn set_output_style(&self, style: Option<OutputStyle>) {
        self.change_code(CodeChange::OutputStyle(style));
    }

    pub fn set_prompt_suggestions(&self, on: bool) {
        self.change_code(CodeChange::PromptSuggestions(on));
    }

    pub fn set_allow_bypass_mode(&self, on: bool) {
        self.change_code(CodeChange::AllowBypass(on));
    }

    // Saves one Code tab control at once. The shown value follows the save, so a failed save leaves
    // the control where it was and shows the error.
    fn change_code(&self, change: CodeChange) {
        let usecase = (*self.usecase.peek()).clone();
        let this = *self;
        // Pinned to the root scope, like the other saves: switching tab must not cancel it.
        dioxus::core::spawn_forever(async move {
            match usecase.save_code_settings(|config| apply_code_change(config, change)).await {
                Ok(config) => this.after_save_code(config),
                Err(error) => this.fail(SettingsSection::Code, error.to_string()),
            }
        });
    }

    // After a Code save only its four fields changed; leave unsaved Connection edits alone.
    fn after_save_code(&self, config: AppConfig) {
        let mut saved = self.saved;
        let mut status = self.status;
        {
            let mut current = saved.write();
            current.default_transcript_view = config.default_transcript_view;
            current.output_style = config.output_style;
            current.prompt_suggestions = config.prompt_suggestions;
            current.allow_bypass_mode = config.allow_bypass_mode;
        }
        status.set(Some((SettingsSection::Code, SettingsStatus::Saved)));
    }
```

Export `SettingsSection::Code` already via the enum. In `chat_window.rs:55` change the call to
`let settings = use_settings(); … let mode = workspace.transcript_view(id, settings.default_transcript_view());` (import `use_settings` from `forge_features`).

- [ ] **Step 7: Run to see pass**

Run: `cargo test -p forge-features -p forge-application && cargo build --workspace`
Expected: PASS and a clean build (the new `SettingsSection::Code` makes `settings_overlay.rs`'s `match` non-exhaustive: add a temporary arm `SettingsSection::Code => rsx! {}` there so the workspace builds; Task 6 replaces it).

- [ ] **Step 8: Commit**

```bash
git add -A crates
git commit -m "feat: Settings saves the Code tab controls at once, and a session without its own transcript view follows the default

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The mode picker hides bypass while the switch is off

**Files:**
- Modify: `crates/forge-features/src/permission_mode.rs` (`mode_rows`, tests), `crates/forge-features/src/lib.rs` (no change unless the export list needs it), `crates/forge-widgets/src/composer_controls.rs:116-170`

**Interfaces:**
- Consumes: `Settings::allow_bypass_mode()` (Task 4).
- Produces: `mode_rows(include_bypass: bool) -> Vec<ModeRow>`: the four modes, plus the bypass row when `include_bypass`.

- [ ] **Step 1: Write the failing tests**

In `permission_mode.rs` `mod tests` (read the existing tests that call `mode_rows()` and update their calls to `mode_rows(true)`), add:

```rust
    #[test]
    fn without_the_switch_the_picker_has_no_bypass_row() {
        let rows = mode_rows(false);
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().all(|row| row.mode != PermissionMode::BypassPermissions));
    }

    #[test]
    fn with_the_switch_bypass_is_the_last_row_below_a_divider() {
        let rows = mode_rows(true);
        let last = rows.last().unwrap();
        assert_eq!(last.mode, PermissionMode::BypassPermissions);
        assert!(last.divider_before);
    }

    #[test]
    fn the_four_modes_keep_their_number_keys_with_or_without_bypass() {
        for include in [false, true] {
            let keys: Vec<_> = mode_rows(include).iter().filter_map(|row| row.key).collect();
            assert_eq!(keys, vec![1, 2, 3, 4]);
        }
    }
```

- [ ] **Step 2: Run to see failure**

Run: `cargo test -p forge-features permission_mode`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**

`permission_mode.rs`: change the function to

```rust
/// The modes in Claude Code desktop's order: four with number keys, then bypass below a divider.
/// The bypass row is left out unless `include_bypass` (the Settings switch, or the session is
/// already in that mode).
pub fn mode_rows(include_bypass: bool) -> Vec<ModeRow> {
    let row = |mode, label, description, key, divider_before| ModeRow { mode, label, description, key, divider_before };
    let mut rows = vec![
        row(PermissionMode::Auto, "Auto", "Claude handles permission decisions", Some(1), false),
        row(PermissionMode::Default, "Ask before edits", "Always ask before making changes", Some(2), false),
        row(PermissionMode::AcceptEdits, "Accept edits", "Automatically accept all file edits", Some(3), false),
        row(PermissionMode::Plan, "Plan mode", "Make a plan before changing anything", Some(4), false),
    ];
    if include_bypass {
        rows.push(row(PermissionMode::BypassPermissions, "Bypass permissions", "Skip every permission check", None, true));
    }
    rows
}
```

`composer_controls.rs` `ModePickerView`: add at the top of the component `let settings = use_settings();` (import from `forge_features`) and replace `let rows = mode_rows();` with

```rust
    // A session already in bypass mode keeps its row, selected, even when the switch is off.
    let rows = mode_rows(settings.allow_bypass_mode() || current == PermissionMode::BypassPermissions);
```

Fix any other caller of `mode_rows()` the compiler lists.

- [ ] **Step 4: Run to see pass**

Run: `cargo test -p forge-features && cargo build --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A crates
git commit -m "feat: the mode picker leaves out bypass permissions unless Settings allows it or the session is already in that mode

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: The Code tab

**Files:**
- Modify: `crates/forge-widgets/src/settings_overlay.rs` (nav button, `match`, new `CodeSection`)

**Interfaces:**
- Consumes: `Settings` getters/setters (Task 4), `OutputStyle`, `TranscriptView` (Task 1), `forge_shared::{Select, SettingRow, Switch}` (existing; `Select` takes `value: String`, `options: Vec<(String, String)>`, `onchange: EventHandler<String>` as in `ModelSection`).

- [ ] **Step 1: Add the tab and the section**

In `SettingsOverlay`, add a nav button after "Model":

```rust
                    button {
                        class: if section == SettingsSection::Code { "settings-nav-item active" } else { "settings-nav-item" },
                        onclick: move |_| settings.set_section(SettingsSection::Code),
                        "Code"
                    }
```

replace the temporary arm from Task 4 with `SettingsSection::Code => rsx! { CodeSection {} },`, add imports (`use forge_domain::{OutputStyle, TranscriptView};` — `forge-widgets` must list `forge-domain` in `Cargo.toml`; it does for `PermissionMode`), and add:

```rust
const DEFAULT_STYLE_VALUE: &str = "default";

#[component]
fn CodeSection() -> Element {
    let settings = use_settings();
    let view = settings.default_transcript_view();
    let style_value = settings.output_style().map_or(DEFAULT_STYLE_VALUE, OutputStyle::name).to_string();
    let mut style_options = vec![(DEFAULT_STYLE_VALUE.to_string(), "Default".to_string())];
    style_options.extend(OutputStyle::ALL.iter().map(|style| (style.name().to_string(), style.name().to_string())));
    rsx! {
        h2 { "Code" }
        SettingRow {
            title: "Default transcript view",
            description: "The view sessions open in. The view menu of a session changes that session only.",
            div { class: "view-switch", role: "radiogroup", "aria-label": "Default transcript view",
                for option in TranscriptView::ALL {
                    button {
                        key: "{option.label()}",
                        class: if option == view { "view-option selected" } else { "view-option" },
                        role: "radio",
                        "aria-checked": "{option == view}",
                        onclick: move |_| settings.set_default_transcript_view(option),
                        "{option.label()}"
                    }
                }
            }
        }
        SettingRow { title: "Output style", description: "Applies to new sessions.",
            Select {
                value: style_value,
                options: style_options,
                onchange: move |value: String| settings.set_output_style(OutputStyle::from_name(&value)),
            }
        }
        SettingRow {
            title: "Prompt suggestions",
            description: "Show a suggested next prompt in the message box after Claude answers. Suggestions use a little more of your usage.",
            Switch { on: settings.prompt_suggestions(), onchange: move |on| settings.set_prompt_suggestions(on) }
        }
        SettingRow {
            title: "Allow bypass permissions mode",
            description: "Adds Bypass permissions to the mode menu, which skips every permission check in a session. A session that is already in that mode keeps it.",
            Switch { on: settings.allow_bypass_mode(), onchange: move |on| settings.set_allow_bypass_mode(on) }
        }
        div { class: "hint", style: "margin-top: 12px;",
            "Sessions that are already open keep what they started with; a change applies to the next new session."
        }
    }
}
```

`Switch`'s `onchange` receives the value the click asks for; the `view-switch`/`view-option` classes are the ones `ChatWindow` uses.

- [ ] **Step 2: Build and run the whole suite**

Run: `cargo build --workspace && cargo test --workspace && (cd sidecar && npx vitest run)`
Expected: all PASS. (Widgets have no unit tests; the behaviour is checked in Task 8.)

- [ ] **Step 3: Commit**

```bash
git add -A crates
git commit -m "feat: the Code tab of Settings with default view, output style, prompt suggestions and the bypass switch

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Check the output style with the real SDK

**Files:**
- Create: `sidecar/scripts/check_output_style.mjs`

The SDK's `system/init` message reports `output_style` and `available_output_styles`. This script starts a query with `settings: { outputStyle }` for each style and prints what the SDK says it is using, so the five names are proved against the real thing and not just against our own tests.

- [ ] **Step 1: Write the script**

```js
// Prints, for each output style name Forge offers, the style the real SDK reports in its init
// message, and the styles it lists as available. Opens a query per style and closes it without
// sending a message to the model.
import { query } from "@anthropic-ai/claude-agent-sdk";

const names = [undefined, "Concise", "Explanatory", "Learning", "Proactive"];

for (const name of names) {
  const controller = new AbortController();
  async function* never() {
    await new Promise((resolve) => controller.signal.addEventListener("abort", resolve));
  }
  const handle = query({
    prompt: never(),
    options: {
      cwd: process.cwd(),
      abortController: controller,
      settingSources: ["project"],
      ...(name ? { settings: { outputStyle: name } } : {}),
    },
  });
  try {
    const init = await Promise.race([
      (async () => {
        const info = await handle.initializationResult();
        return info;
      })(),
      new Promise((_, reject) => setTimeout(() => reject(new Error("no answer in 60 s")), 60000)),
    ]);
    console.log(JSON.stringify({ asked: name ?? "(none)", output_style: init.output_style ?? null, available: init.available_output_styles ?? null }));
  } catch (error) {
    console.log(JSON.stringify({ asked: name ?? "(none)", error: String(error) }));
  } finally {
    controller.abort();
  }
}
process.exit(0);
```

- [ ] **Step 2: Run it**

Run: `cd sidecar && node scripts/check_output_style.mjs`
Expected: five lines. For each asked name, `output_style` equals the name asked (or the list in `available` contains it). If `initializationResult()` lacks `output_style` in this SDK version, read the field name in `sdk.d.ts` near line 4572 (`output_style`) and the control response type, and change the script to read it from the init message; do not guess.

- [ ] **Step 3: Act on the result**

If a name is reported as unknown or falls back to the default: correct `OutputStyle::name()` (Task 1) and the tests that pin the names to what the SDK reports, and record the finding. If all five are accepted, write the five lines under a "Checked with the real SDK" note in the plan file's last section (Task 9).

- [ ] **Step 4: Commit**

```bash
git add sidecar/scripts/check_output_style.mjs
git commit -m "chore: a script that asks the real SDK which output style it uses for each name Forge offers

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Check in the real window

Use the harness from the memory notes "Driving the Forge app visually" (screencapture, Quartz clicks, `FORGE_CLAUDE_HOME` fixture) and the lessons of Plans 16, 19 and 30 (click scale, capture after each step, close drop-downs by clicking their button again).

- [ ] **Step 1: Build and start the app with a fixture home**

Run the app the way the harness notes describe, with `FORGE_CLAUDE_HOME` pointing at a fixture that holds one past session and an empty Forge config.

- [ ] **Step 2: Walk through the tab**

Record a screenshot for each:
1. Open Settings, open the Code tab: the four controls, defaults Normal / Default / on / off.
2. Pick Thinking as the default view; close Settings; open a new session: the chat header shows Thinking selected. Pick Normal in that session's header, then change the default to Detailed: the session stays Normal, a second new session shows Detailed.
3. Open the mode picker of a session: no "Bypass permissions" row. Turn the switch on: the row appears (reopen the picker). Choose it, confirm, and turn the switch off again: the session's picker still shows Bypass selected; a new session's picker has no such row.
4. Turn prompt suggestions off, start a new session, send a message that usually draws a suggestion: the message box stays on its normal placeholder after the answer. Turn it on and see a suggestion again.
5. Choose "Concise" and start a session: ask for an explanation and see a short answer; the same prompt with the default style gives a longer one.
6. Quit and start the app: all four values are as left; open `config.json` and see the four fields.

- [ ] **Step 3: Fix what the walk-through finds**

Each fix gets its own test where the logic is pure, and a commit. Note anything deferred.

---

### Task 9: Docs and finish

**Files:**
- Modify: `docs/superpowers/roadmap/backlog.md` (SET-04, SET-05, SET-06, SET-09 rows: Forge `has`, Plan `Plan 32A (built)`, notes), `sidecar/docs/sdk-types-notes.md` if it lists the options used, `docs/superpowers/plans/2026-10-09-code-settings.md` (a "Checked with the real SDK" note from Task 7)

- [ ] **Step 1: Update the backlog rows**

For each of the four rows set the Forge column to `has`, the Plan column to `Plan 32A (built)`, and the note to what was built in one sentence (for SET-05: "default only; the per-session override stays SES-04 won't?"; for SET-09: "the SDK is also told, so it refuses the mode when off").

- [ ] **Step 2: Run everything once more**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets && (cd sidecar && npx vitest run && npm run typecheck)`
Expected: PASS and no new clippy warnings.

- [ ] **Step 3: Commit, then ask how to integrate**

```bash
git add -A docs sidecar/docs
git commit -m "docs: Plan 32A is built; SET-04, 05, 06, 09 marked in the backlog

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

Then use superpowers:finishing-a-development-branch (the base branch is master per the Plan 20 note; the git status in this session names `main` as the main branch, so confirm which one before merging).

## Checked with the real SDK

Asked name -> reported `output_style` in the init message; listed in `available_output_styles`: yes.

- Concise -> Concise; listed in available_output_styles: yes
- Explanatory -> Explanatory; listed in available_output_styles: yes
- Learning -> Learning; listed in available_output_styles: yes
- Proactive -> Proactive; listed in available_output_styles: yes
- (none, "Default") -> default; listed in available_output_styles: yes

Task 8 real-window walk-through: all steps passed. The effect of Concise on answer length was not observable with the probe prompt; the wiring is confirmed in the transcript (output_style attachments).
