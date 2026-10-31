# Forge Design Polish and Settings Overlay Implementation Plan (Plan 6)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restyle the Plan 5 workspace into a flat, Claude-Code-desktop-like UI and add a `⌘,` settings overlay (Connection, Model) so credentials and the default model are set in the app, not in `config.json`.

**Architecture:** `AppConfig` gains `model`; a thin `SettingsUseCase` in `forge-application` does load/modify/save over the existing `ConfigRepositoryPort`. A `Settings` handle in `forge-features` (Copy, signals, like `Workspace`) drives a `SettingsOverlay` widget; pure logic (draft dirty check, model catalog, shortcut) lives in small testable modules. `Workspace::start_new` reads the saved model at start time. The theme is rewritten in `theme.css`, keeping existing class names.

**Tech Stack:** Rust 2021, Dioxus 0.7.10 desktop, tokio, mockall (existing test mocks).

**Spec:** `docs/superpowers/specs/2026-09-27-forge-design.md` §16 (and §15.11 for the marker complaint, §15.12 for what stays deferred).

## Global Constraints

- Hexagonal crate graph is enforced by `Cargo.toml` dependencies: `forge-domain` depends on nothing of ours; `forge-application` on domain + ports only; `forge-features`/`forge-widgets`/`forge-pages` must not name a concrete adapter. Only `forge-app/src/main.rs` names adapters.
- `AppConfig` field additions must be `#[serde(default)]` so an existing `config.json` still loads (spec §16.2).
- Empty or whitespace-only `base_url` / `api_key` / `model` is stored as `None`; values are trimmed (spec §16.2).
- Saving settings must never erase `enabled_skills` / `enabled_plugins` / `enabled_mcp_servers` (spec §16.2).
- Connection fields save with a Save button; the model saves the moment it is chosen (spec §16.1).
- Saved settings apply to sessions started afterwards; running sessions keep their environment (spec §16.1).
- Sidebar markers differ by shape and motion, not colour alone; animation respects `prefers-reduced-motion` (spec §16.5).
- Model IDs (copy verbatim): `claude-fable-5-1`, `claude-opus-5-5`, `claude-sonnet-5-5`, `claude-haiku-4-5-20251001`.
- UI text is English (the existing UI is English: "New session", "Dismiss", "Message Claude…").
- Every commit message ends with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Test commands: `cargo test -p <crate>`; full check `cargo test --workspace` (baseline before this plan: 249 tests + sidecar 19; the `block v0.1.6` future-incompat notice is expected).

## Review Focus

Inputs the spec implies but that are easy to get wrong, most likely first. Each has a test in the task named.

1. An API key pasted with a trailing newline or spaces must be stored trimmed, not rejected or kept dirty (Task 1 normaliser test; Task 3 draft test).
2. A `config.json` that is broken (not valid JSON) must not make Save impossible: Save replaces it (Task 2 test).
3. A `config.json` holding hand-written `enabled_*` lists must keep them after saving a key or a model (Task 2 tests).
4. A saved model that is not in the built-in table (custom ID) must still be shown and selected, not silently replaced by "Default" (Task 3 catalog test).
5. A failed save must keep what the person typed and keep the overlay open; the Save button must stay enabled (Task 3 draft test "stays dirty until the saved config matches"; Task 4 code only ever sets `status` on failure).

---

### Task 1: Domain: `AppConfig.model` and the setting normaliser

**Files:**
- Modify: `crates/forge-domain/src/config.rs`
- Modify: `crates/forge-domain/src/lib.rs`
- Modify (only if the compiler says a struct literal is missing the field): tests in `crates/forge-adapter-config/src/lib.rs`

**Interfaces:**
- Produces: `AppConfig.model: Option<String>`; `pub fn normalize_setting(value: &str) -> Option<String>` exported from `forge_domain`.

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests` in `crates/forge-domain/src/config.rs`:

```rust
    #[test]
    fn a_config_file_without_a_model_loads_with_none() {
        let json = r#"{"base_url": null, "api_key": null, "enabled_skills": ["a"]}"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.model, None);
        assert_eq!(config.enabled_skills, vec!["a".to_string()]);
    }

    #[test]
    fn the_model_round_trips_through_json() {
        let config = AppConfig { model: Some("claude-sonnet-5-5".to_string()), ..AppConfig::default() };
        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(back.model.as_deref(), Some("claude-sonnet-5-5"));
    }

    #[test]
    fn normalize_setting_trims_and_turns_blank_into_none() {
        assert_eq!(normalize_setting(""), None);
        assert_eq!(normalize_setting("   \n"), None);
        assert_eq!(normalize_setting(" sk-ant-1 \n"), Some("sk-ant-1".to_string()));
        assert_eq!(normalize_setting("https://x.test"), Some("https://x.test".to_string()));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p forge-domain config`
Expected: FAIL to compile (`no field model`, `cannot find function normalize_setting`).

- [ ] **Step 3: Implement**

In `crates/forge-domain/src/config.rs`, add the field after `api_key` and the function before `#[cfg(test)]`:

```rust
    pub api_key: Option<String>,
    /// The model new sessions start with; `None` lets the SDK choose.
    #[serde(default)]
    pub model: Option<String>,
```

```rust
/// What a settings text field is stored as: trimmed, and `None` when nothing is left.
pub fn normalize_setting(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
```

In `crates/forge-domain/src/lib.rs` change the config export to `pub use config::{normalize_setting, AppConfig};`.

The existing test `app_config_round_trips_through_json` builds a full literal: add `model: None,` to it.

- [ ] **Step 4: Run and fix other literals**

Run: `cargo test --workspace --no-run 2>&1 | grep -B2 -A6 "missing field"`
For each struct literal of `AppConfig` that lists every field (e.g. around `crates/forge-adapter-config/src/lib.rs:110`), add `model: None,` (or `..AppConfig::default()`).
Then run: `cargo test -p forge-domain -p forge-adapter-config`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add the default model to AppConfig and a setting normaliser

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Application: `SettingsUseCase`

**Files:**
- Create: `crates/forge-application/src/use_cases/settings.rs`
- Modify: `crates/forge-application/src/use_cases/mod.rs`
- Modify: `crates/forge-application/src/lib.rs`

**Interfaces:**
- Consumes: `ConfigRepositoryPort` (`load`, `save`), `normalize_setting`.
- Produces:
  ```rust
  pub struct SettingsUseCase { /* Arc<dyn ConfigRepositoryPort> */ }
  impl SettingsUseCase {
      pub fn new(config: Arc<dyn ConfigRepositoryPort>) -> Self;
      pub async fn load(&self) -> Result<AppConfig, ConfigError>;
      pub async fn saved_model(&self) -> Result<Option<String>, ConfigError>;
      pub async fn save_connection(&self, base_url: &str, api_key: &str) -> Result<AppConfig, ConfigError>;
      pub async fn save_model(&self, model: &str) -> Result<AppConfig, ConfigError>;
  }
  ```
  `save_*` load the current config, change only their own fields, save, and return the saved config. A config file that fails to *parse* is treated as empty (Save replaces it); other load errors are returned.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-application/src/use_cases/settings.rs` with the tests first (implementation added in Step 3):

```rust
use std::sync::Arc;

use forge_domain::{normalize_setting, AppConfig};
use forge_ports::{ConfigError, ConfigRepositoryPort};

#[cfg(test)]
mod tests {
    use super::*;
    use forge_ports::MockConfigRepositoryPort;

    fn with_lists() -> AppConfig {
        AppConfig {
            base_url: Some("https://old.test".to_string()),
            api_key: Some("old-key".to_string()),
            model: Some("old-model".to_string()),
            enabled_skills: vec!["brainstorming".to_string()],
            enabled_plugins: vec!["p@m".to_string()],
            enabled_mcp_servers: vec!["playwright".to_string()],
        }
    }

    #[tokio::test]
    async fn save_connection_replaces_only_the_url_and_the_key() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(with_lists()));
        mock.expect_save()
            .withf(|c: &AppConfig| {
                c.base_url.as_deref() == Some("https://new.test")
                    && c.api_key.as_deref() == Some("new-key")
                    && c.model.as_deref() == Some("old-model")
                    && c.enabled_skills == vec!["brainstorming".to_string()]
                    && c.enabled_plugins == vec!["p@m".to_string()]
                    && c.enabled_mcp_servers == vec!["playwright".to_string()]
            })
            .times(1)
            .returning(|_| Ok(()));

        let saved = SettingsUseCase::new(Arc::new(mock))
            .save_connection(" https://new.test ", "new-key\n")
            .await
            .unwrap();
        assert_eq!(saved.api_key.as_deref(), Some("new-key"));
    }

    #[tokio::test]
    async fn a_blank_field_clears_the_value() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(with_lists()));
        mock.expect_save()
            .withf(|c: &AppConfig| c.base_url.is_none() && c.api_key.is_none())
            .times(1)
            .returning(|_| Ok(()));

        SettingsUseCase::new(Arc::new(mock)).save_connection("", "  ").await.unwrap();
    }

    #[tokio::test]
    async fn save_model_replaces_only_the_model() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(with_lists()));
        mock.expect_save()
            .withf(|c: &AppConfig| {
                c.model.as_deref() == Some("claude-opus-5-5")
                    && c.api_key.as_deref() == Some("old-key")
                    && c.enabled_skills == vec!["brainstorming".to_string()]
            })
            .times(1)
            .returning(|_| Ok(()));

        SettingsUseCase::new(Arc::new(mock)).save_model("claude-opus-5-5").await.unwrap();
    }

    #[tokio::test]
    async fn an_empty_model_means_the_sdk_default() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(with_lists()));
        mock.expect_save().withf(|c: &AppConfig| c.model.is_none()).times(1).returning(|_| Ok(()));

        let saved = SettingsUseCase::new(Arc::new(mock)).save_model("").await.unwrap();
        assert_eq!(saved.model, None);
    }

    #[tokio::test]
    async fn a_config_file_that_does_not_parse_is_replaced_on_save() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load()
            .times(1)
            .returning(|| Err(ConfigError::ParseFailed("expected value".to_string())));
        mock.expect_save()
            .withf(|c: &AppConfig| c.api_key.as_deref() == Some("k") && c.enabled_skills.is_empty())
            .times(1)
            .returning(|_| Ok(()));

        SettingsUseCase::new(Arc::new(mock)).save_connection("", "k").await.unwrap();
    }

    #[tokio::test]
    async fn a_read_error_is_returned_and_nothing_is_saved() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Err(ConfigError::ReadFailed("denied".to_string())));
        mock.expect_save().times(0);

        assert!(SettingsUseCase::new(Arc::new(mock)).save_model("m").await.is_err());
    }

    #[tokio::test]
    async fn a_write_error_is_returned() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(AppConfig::default()));
        mock.expect_save()
            .times(1)
            .returning(|_| Err(ConfigError::WriteFailed("disk full".to_string())));

        assert!(SettingsUseCase::new(Arc::new(mock)).save_model("m").await.is_err());
    }

    #[tokio::test]
    async fn saved_model_reads_the_model_from_the_config() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(with_lists()));

        let model = SettingsUseCase::new(Arc::new(mock)).saved_model().await.unwrap();
        assert_eq!(model.as_deref(), Some("old-model"));
    }

    #[tokio::test]
    async fn saved_model_fails_when_the_config_cannot_be_loaded() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load()
            .times(1)
            .returning(|| Err(ConfigError::ParseFailed("bad".to_string())));

        assert!(SettingsUseCase::new(Arc::new(mock)).saved_model().await.is_err());
    }
}
```

Register the module: in `use_cases/mod.rs` add `mod settings;` and `pub use settings::SettingsUseCase;`; in `lib.rs` add `SettingsUseCase` to the `pub use use_cases::{...}` list.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p forge-application settings`
Expected: FAIL to compile (`cannot find type SettingsUseCase`).

- [ ] **Step 3: Implement**

Insert above `#[cfg(test)]` in `settings.rs`:

```rust
/// Reads and changes the settings the UI owns (`base_url`, `api_key`, `model`). Every save loads
/// the current config first, so the `enabled_*` lists are never lost.
pub struct SettingsUseCase {
    config: Arc<dyn ConfigRepositoryPort>,
}

impl SettingsUseCase {
    pub fn new(config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { config }
    }

    pub async fn load(&self) -> Result<AppConfig, ConfigError> {
        self.config.load().await
    }

    /// The model a new session should start with (`None`: the SDK chooses).
    pub async fn saved_model(&self) -> Result<Option<String>, ConfigError> {
        Ok(self.config.load().await?.model)
    }

    pub async fn save_connection(&self, base_url: &str, api_key: &str) -> Result<AppConfig, ConfigError> {
        let mut config = self.load_for_update().await?;
        config.base_url = normalize_setting(base_url);
        config.api_key = normalize_setting(api_key);
        self.config.save(&config).await?;
        Ok(config)
    }

    pub async fn save_model(&self, model: &str) -> Result<AppConfig, ConfigError> {
        let mut config = self.load_for_update().await?;
        config.model = normalize_setting(model);
        self.config.save(&config).await?;
        Ok(config)
    }

    // A file that is not valid JSON cannot be repaired by the settings screen, so saving replaces
    // it; any other failure (for example a permission error) is reported, never overwritten.
    async fn load_for_update(&self) -> Result<AppConfig, ConfigError> {
        match self.config.load().await {
            Err(ConfigError::ParseFailed(_)) => Ok(AppConfig::default()),
            other => other,
        }
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p forge-application`
Expected: PASS (all previous tests plus 9 new).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add SettingsUseCase over the config port

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Pure UI logic: model catalog, connection draft, settings shortcut

**Files:**
- Create: `crates/forge-entities/src/model_catalog.rs`
- Modify: `crates/forge-entities/src/lib.rs`
- Create: `crates/forge-features/src/settings_draft.rs`
- Modify: `crates/forge-features/src/shortcut.rs`
- Modify: `crates/forge-features/src/lib.rs`

**Interfaces:**
- Produces (entities):
  ```rust
  pub const DEFAULT_MODEL_VALUE: &str = "__default";
  pub const KNOWN_MODELS: [(&str, &str); 4];               // (id, label)
  pub fn model_options(saved: Option<&str>) -> Vec<(String, String)>; // (value, label); default first
  pub fn model_label(saved: Option<&str>) -> String;
  pub fn model_value(saved: Option<&str>) -> String;        // the option value to select
  pub fn model_from_value(value: &str) -> String;           // "" for the default, else the id
  ```
- Produces (features): `ConnectionDraft { base_url: String, api_key: String }` with `from_config(&AppConfig) -> Self` and `is_dirty(&self, saved: &AppConfig) -> bool`; `Shortcut::OpenSettings`.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-entities/src/model_catalog.rs`:

```rust
/// The value of the "Default" option in a model list (an empty model is stored as `None`).
pub const DEFAULT_MODEL_VALUE: &str = "__default";

/// The models offered in Settings: `(id, label)`. Nothing lists models at runtime, so this is the
/// one place to edit when models change.
pub const KNOWN_MODELS: [(&str, &str); 4] = [
    ("claude-fable-5-1", "Fable 5.1"),
    ("claude-opus-5-5", "Opus 5.5"),
    ("claude-sonnet-5-5", "Sonnet 5.5"),
    ("claude-haiku-4-5-20251001", "Haiku 4.5"),
];

const DEFAULT_LABEL: &str = "Default (chosen by the SDK)";

fn is_known(id: &str) -> bool {
    KNOWN_MODELS.iter().any(|(known, _)| *known == id)
}

/// `(value, label)` for a model dropdown: the default, the known models, and — when the saved
/// model is something else (typed by hand) — that model, so it is shown rather than hidden.
pub fn model_options(saved: Option<&str>) -> Vec<(String, String)> {
    let mut options = vec![(DEFAULT_MODEL_VALUE.to_string(), DEFAULT_LABEL.to_string())];
    options.extend(KNOWN_MODELS.iter().map(|(id, label)| (id.to_string(), label.to_string())));
    if let Some(id) = saved.filter(|id| !is_known(id)) {
        options.push((id.to_string(), format!("Custom: {id}")));
    }
    options
}

/// The option value that represents the saved model.
pub fn model_value(saved: Option<&str>) -> String {
    saved.map(str::to_string).unwrap_or_else(|| DEFAULT_MODEL_VALUE.to_string())
}

/// A short name for the saved model, for read-only display.
pub fn model_label(saved: Option<&str>) -> String {
    match saved {
        None => "Default model".to_string(),
        Some(id) => KNOWN_MODELS
            .iter()
            .find(|(known, _)| *known == id)
            .map(|(_, label)| label.to_string())
            .unwrap_or_else(|| id.to_string()),
    }
}

/// What to save for a chosen option value: empty for the default.
pub fn model_from_value(value: &str) -> String {
    if value == DEFAULT_MODEL_VALUE {
        String::new()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_comes_first_then_the_known_models() {
        let options = model_options(None);
        assert_eq!(options[0].0, DEFAULT_MODEL_VALUE);
        assert_eq!(options.len(), 1 + KNOWN_MODELS.len());
        assert!(options.iter().any(|(value, label)| value == "claude-opus-5-5" && label == "Opus 5.5"));
    }

    #[test]
    fn a_saved_model_that_is_not_in_the_table_is_still_offered_and_selected() {
        let options = model_options(Some("my-private-model"));
        assert!(options.contains(&("my-private-model".to_string(), "Custom: my-private-model".to_string())));
        assert_eq!(model_value(Some("my-private-model")), "my-private-model");
    }

    #[test]
    fn a_known_saved_model_is_not_listed_twice() {
        let options = model_options(Some("claude-sonnet-5-5"));
        assert_eq!(options.iter().filter(|(value, _)| value == "claude-sonnet-5-5").count(), 1);
    }

    #[test]
    fn no_saved_model_selects_the_default_option() {
        assert_eq!(model_value(None), DEFAULT_MODEL_VALUE);
    }

    #[test]
    fn labels_use_the_friendly_name_for_known_models_and_the_id_otherwise() {
        assert_eq!(model_label(None), "Default model");
        assert_eq!(model_label(Some("claude-haiku-4-5-20251001")), "Haiku 4.5");
        assert_eq!(model_label(Some("my-private-model")), "my-private-model");
    }

    #[test]
    fn choosing_the_default_option_saves_an_empty_model() {
        assert_eq!(model_from_value(DEFAULT_MODEL_VALUE), "");
        assert_eq!(model_from_value("claude-opus-5-5"), "claude-opus-5-5");
    }
}
```

In `crates/forge-entities/src/lib.rs` add `mod model_catalog;` and
`pub use model_catalog::{model_from_value, model_label, model_options, model_value, DEFAULT_MODEL_VALUE, KNOWN_MODELS};`.

Create `crates/forge-features/src/settings_draft.rs`:

```rust
use forge_domain::{normalize_setting, AppConfig};

/// What the Connection tab's two text fields currently hold (not yet saved).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConnectionDraft {
    pub base_url: String,
    pub api_key: String,
}

impl ConnectionDraft {
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            base_url: config.base_url.clone().unwrap_or_default(),
            api_key: config.api_key.clone().unwrap_or_default(),
        }
    }

    /// Whether saving would change the stored config. Compared the way saving stores the text
    /// (trimmed, blank = none), so a stray trailing newline is not a difference.
    pub fn is_dirty(&self, saved: &AppConfig) -> bool {
        normalize_setting(&self.base_url) != saved.base_url || normalize_setting(&self.api_key) != saved.api_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved() -> AppConfig {
        AppConfig {
            base_url: Some("https://a.test".to_string()),
            api_key: Some("k".to_string()),
            ..AppConfig::default()
        }
    }

    #[test]
    fn a_draft_loaded_from_the_config_is_clean() {
        let config = saved();
        assert!(!ConnectionDraft::from_config(&config).is_dirty(&config));
        let empty = AppConfig::default();
        assert!(!ConnectionDraft::from_config(&empty).is_dirty(&empty));
    }

    #[test]
    fn editing_a_field_makes_it_dirty() {
        let config = saved();
        let mut draft = ConnectionDraft::from_config(&config);
        draft.api_key = "other".to_string();
        assert!(draft.is_dirty(&config));
    }

    #[test]
    fn whitespace_around_the_same_value_is_not_a_change() {
        let config = saved();
        let mut draft = ConnectionDraft::from_config(&config);
        draft.api_key = "k\n".to_string();
        draft.base_url = "  https://a.test ".to_string();
        assert!(!draft.is_dirty(&config));
    }

    #[test]
    fn clearing_a_saved_value_is_a_change_and_blank_against_none_is_not() {
        let config = saved();
        let mut draft = ConnectionDraft::from_config(&config);
        draft.api_key.clear();
        assert!(draft.is_dirty(&config));
        assert!(!ConnectionDraft { base_url: "  ".to_string(), api_key: String::new() }.is_dirty(&AppConfig::default()));
    }

    #[test]
    fn it_stays_dirty_until_the_saved_config_matches() {
        // A failed save leaves `saved` unchanged, so the Save button must stay enabled.
        let config = saved();
        let draft = ConnectionDraft { base_url: "https://b.test".to_string(), api_key: "k".to_string() };
        assert!(draft.is_dirty(&config));
        let after_success = AppConfig { base_url: Some("https://b.test".to_string()), ..config };
        assert!(!draft.is_dirty(&after_success));
    }
}
```

In `crates/forge-features/src/shortcut.rs` add to `mod tests`:

```rust
    #[test]
    fn cmd_comma_opens_settings_on_a_mac_and_ctrl_comma_elsewhere() {
        assert_eq!(shortcut_for(",", false, true, false, true), Some(Shortcut::OpenSettings));
        assert_eq!(shortcut_for(",", true, false, false, false), Some(Shortcut::OpenSettings));
        assert_eq!(shortcut_for(",", true, false, false, true), None, "Ctrl+, on a Mac");
        assert_eq!(shortcut_for(",", false, true, true, true), None, "Cmd+Shift+,");
    }
```

In `crates/forge-features/src/lib.rs` add `mod settings_draft;` and `pub use settings_draft::ConnectionDraft;`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p forge-entities -p forge-features 2>&1 | tail -20`
Expected: FAIL (`no variant OpenSettings`; everything else compiles once the `mod` lines exist).

- [ ] **Step 3: Implement the shortcut**

In `shortcut.rs`: add `OpenSettings,` to the `Shortcut` enum, add `"," => Some(Shortcut::OpenSettings),` to the `match` in `shortcut_for`, and extend the doc comment with "Cmd+, opens settings".

In `crates/forge-features/src/workspace.rs`, in `on_key`'s `match shortcut`, add a first arm so the crate compiles (the Settings handle deals with it in Task 4):

```rust
            // Opening settings belongs to `Settings::on_key`; the page offers it the event next.
            Shortcut::OpenSettings => return false,
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p forge-entities -p forge-features`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: model catalog, connection draft and the settings shortcut

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Wiring: `Settings` handle, `Workspace` changes, composition root

Compile-checked only (signals need a live Dioxus runtime; the pure logic they call is tested in Tasks 1–3). Runtime behaviour is verified in Task 8.

**Files:**
- Create: `crates/forge-features/src/settings.rs`
- Modify: `crates/forge-features/src/workspace.rs`
- Modify: `crates/forge-features/src/lib.rs`
- Modify: `crates/forge-features/src/new_session.rs`
- Modify: `crates/forge-app/src/main.rs`

**Interfaces:**
- Consumes: `SettingsUseCase` (Task 2), `ConnectionDraft` (Task 3), `model_label` (Task 3).
- Produces:
  ```rust
  // forge-features
  pub struct WorkspaceServices { pub registry, pub projects, pub sessions, pub settings: Arc<SettingsUseCase> }
  pub enum SettingsSection { Connection, Model }
  pub enum SettingsStatus { Saved, Error(String) }
  #[derive(Clone, Copy)] pub struct Settings { .. }
  pub fn use_settings_provider() -> Settings;   // call in the page, before use_workspace_provider
  pub fn use_settings() -> Settings;
  impl Settings {
      pub fn is_open(&self) -> bool;
      pub fn open(&self);
      pub fn close(&self);
      pub fn section(&self) -> SettingsSection;
      pub fn set_section(&self, section: SettingsSection);
      pub fn base_url(&self) -> String;   pub fn set_base_url(&self, v: String);
      pub fn api_key(&self) -> String;    pub fn set_api_key(&self, v: String);
      pub fn key_visible(&self) -> bool;  pub fn toggle_key_visible(&self);
      pub fn connection_dirty(&self) -> bool;
      pub fn save_connection(&self);
      pub fn model(&self) -> Option<String>;     // the saved model
      pub fn choose_model(&self, value: String);  // `value` as saved: "" = default
      pub fn status(&self) -> Option<SettingsStatus>;
      pub fn on_key(&self, event: &KeyboardEvent) -> bool;
  }
  // Workspace
  pub fn start_new(&self, project: ProjectSummary, mode: PermissionMode, text: String); // model removed
  pub fn new_folder(&self) -> Option<PathBuf>;
  pub fn set_new_folder(&self, folder: Option<PathBuf>);
  pub fn show_new_session_in(&self, folder: PathBuf);
  ```

- [ ] **Step 1: Create `settings.rs`**

```rust
use std::sync::Arc;

use dioxus::prelude::*;
use forge_application::SettingsUseCase;
use forge_domain::AppConfig;

use crate::settings_draft::ConnectionDraft;
use crate::workspace::{shortcut_from_event, WorkspaceServices};
use crate::shortcut::Shortcut;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Connection,
    Model,
}

/// The outcome of the last save, shown under the fields until the next edit or save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsStatus {
    Saved,
    Error(String),
}

/// The settings overlay's state. `Copy` (every field is a signal), like `Workspace`.
#[derive(Clone, Copy)]
pub struct Settings {
    usecase: Signal<Arc<SettingsUseCase>>,
    open: Signal<bool>,
    section: Signal<SettingsSection>,
    saved: Signal<AppConfig>,
    base_url: Signal<String>,
    api_key: Signal<String>,
    key_visible: Signal<bool>,
    status: Signal<Option<SettingsStatus>>,
}

/// Creates the settings state, provides it to every descendant and loads the saved config once.
/// Call it in the page before `use_workspace_provider` (the new-session panel reads the model).
pub fn use_settings_provider() -> Settings {
    let services = use_context::<WorkspaceServices>();
    let settings = use_hook(move || Settings {
        usecase: Signal::new(services.settings.clone()),
        open: Signal::new(false),
        section: Signal::new(SettingsSection::Connection),
        saved: Signal::new(AppConfig::default()),
        base_url: Signal::new(String::new()),
        api_key: Signal::new(String::new()),
        key_visible: Signal::new(false),
        status: Signal::new(None),
    });
    use_context_provider(move || settings);
    use_hook(move || settings.reload());
    settings
}

pub fn use_settings() -> Settings {
    use_context::<Settings>()
}

impl Settings {
    pub fn is_open(&self) -> bool {
        *self.open.read()
    }

    /// Opens the overlay with the fields showing what is stored (discarding earlier edits).
    pub fn open(&self) {
        let mut open = self.open;
        let mut status = self.status;
        let mut key_visible = self.key_visible;
        open.set(true);
        status.set(None);
        key_visible.set(false);
        self.reload();
    }

    pub fn close(&self) {
        let mut open = self.open;
        open.set(false);
    }

    pub fn section(&self) -> SettingsSection {
        *self.section.read()
    }

    pub fn set_section(&self, section: SettingsSection) {
        let mut current = self.section;
        current.set(section);
    }

    pub fn base_url(&self) -> String {
        self.base_url.read().clone()
    }

    pub fn set_base_url(&self, value: String) {
        let mut field = self.base_url;
        field.set(value);
        self.clear_status();
    }

    pub fn api_key(&self) -> String {
        self.api_key.read().clone()
    }

    pub fn set_api_key(&self, value: String) {
        let mut field = self.api_key;
        field.set(value);
        self.clear_status();
    }

    pub fn key_visible(&self) -> bool {
        *self.key_visible.read()
    }

    pub fn toggle_key_visible(&self) {
        let mut visible = self.key_visible;
        let now = *visible.peek();
        visible.set(!now);
    }

    /// Whether the two Connection fields differ from what is stored (enables Save).
    pub fn connection_dirty(&self) -> bool {
        let draft = ConnectionDraft { base_url: self.base_url(), api_key: self.api_key() };
        draft.is_dirty(&self.saved.read())
    }

    /// The stored default model (`None`: the SDK chooses).
    pub fn model(&self) -> Option<String> {
        self.saved.read().model.clone()
    }

    pub fn status(&self) -> Option<SettingsStatus> {
        self.status.read().clone()
    }

    /// Saves the Connection fields. On failure the fields keep what was typed and only the
    /// status changes.
    pub fn save_connection(&self) {
        let usecase = (*self.usecase.peek()).clone();
        let base_url = self.base_url.peek().clone();
        let api_key = self.api_key.peek().clone();
        let this = *self;
        spawn(async move {
            match usecase.save_connection(&base_url, &api_key).await {
                Ok(config) => this.after_save(config),
                Err(error) => this.fail(error.to_string()),
            }
        });
    }

    /// Saves the chosen model at once. `value` is what to store: empty means the SDK default.
    pub fn choose_model(&self, value: String) {
        let usecase = (*self.usecase.peek()).clone();
        let this = *self;
        spawn(async move {
            match usecase.save_model(&value).await {
                Ok(config) => this.after_save_model(config),
                Err(error) => this.fail(error.to_string()),
            }
        });
    }

    /// Opens on ⌘, and closes on Esc; returns whether the key press was one of them.
    pub fn on_key(&self, event: &KeyboardEvent) -> bool {
        if self.is_open() && event.key() == Key::Escape {
            self.close();
            event.prevent_default();
            return true;
        }
        if shortcut_from_event(event) == Some(Shortcut::OpenSettings) {
            self.open();
            event.prevent_default();
            return true;
        }
        false
    }

    // Loads the stored config into `saved` and the two fields.
    fn reload(&self) {
        let usecase = (*self.usecase.peek()).clone();
        let this = *self;
        spawn(async move {
            match usecase.load().await {
                Ok(config) => this.show(config),
                Err(error) => this.fail(error.to_string()),
            }
        });
    }

    fn show(&self, config: AppConfig) {
        let draft = ConnectionDraft::from_config(&config);
        let mut saved = self.saved;
        let mut base_url = self.base_url;
        let mut api_key = self.api_key;
        saved.set(config);
        base_url.set(draft.base_url);
        api_key.set(draft.api_key);
    }

    // After a Connection save: keep the fields as the person typed them, record what is stored.
    fn after_save(&self, config: AppConfig) {
        let mut saved = self.saved;
        let mut status = self.status;
        saved.set(config);
        status.set(Some(SettingsStatus::Saved));
    }

    // After a model save only the model changed; leave any unsaved Connection edits alone.
    fn after_save_model(&self, config: AppConfig) {
        let mut saved = self.saved;
        let mut status = self.status;
        let model = config.model;
        saved.write().model = model;
        status.set(Some(SettingsStatus::Saved));
    }

    fn fail(&self, message: String) {
        let mut status = self.status;
        status.set(Some(SettingsStatus::Error(message)));
    }

    fn clear_status(&self) {
        let mut status = self.status;
        status.set(None);
    }
}
```

- [ ] **Step 2: Change `workspace.rs`**

a) Imports/services: add `use std::path::PathBuf;` and `use forge_application::SettingsUseCase;` (extend the existing `forge_application` import), then:

```rust
pub struct WorkspaceServices {
    pub registry: Arc<SessionRegistry>,
    pub projects: Arc<dyn ProjectRepositoryPort>,
    pub sessions: Arc<dyn SessionRepositoryPort>,
    pub settings: Arc<SettingsUseCase>,
}
```

b) Add field `new_folder: Signal<Option<PathBuf>>,` to `Workspace` and `new_folder: Signal::new(None),` in `use_workspace_provider`.

c) Add methods next to `show_new_session`:

```rust
    /// The folder chosen in the new-session panel (`None`: the first one offered).
    pub fn new_folder(&self) -> Option<PathBuf> {
        self.new_folder.read().clone()
    }

    pub fn set_new_folder(&self, folder: Option<PathBuf>) {
        let mut current = self.new_folder;
        current.set(folder);
    }

    /// Shows the new-session panel with `folder` preselected (the `+` beside a project).
    pub fn show_new_session_in(&self, folder: PathBuf) {
        self.set_new_folder(Some(folder));
        self.show_new_session();
    }
```

d) Replace `start_new`:

```rust
    /// Opens a new session in `project`, on the saved default model, and sends its first message.
    pub fn start_new(&self, project: ProjectSummary, mode: PermissionMode, text: String) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        spawn(async move {
            let model = match services.settings.saved_model().await {
                Ok(model) => model,
                Err(error) => {
                    this.report(format!("failed to load the Forge config: {error}"));
                    return;
                }
            };
            match services.registry.open_new(&project, mode, model) {
                Ok(handle) => {
                    let id = this.attach(handle.clone());
                    handle.send(text);
                    this.select(id);
                }
                Err(error) => this.report(error.to_string()),
            }
        });
    }
```

e) Extract the key decoding out of `on_key` as a crate-visible function (Settings uses it too). Replace the top of `on_key`'s body up to and including the `let Some(shortcut) = ... else { return false };` with `let Some(shortcut) = shortcut_from_event(event) else { return false };` and add below the `impl`:

```rust
/// The shortcut a key press is, if any (platform-aware: Cmd on macOS, Ctrl elsewhere).
pub(crate) fn shortcut_from_event(event: &KeyboardEvent) -> Option<Shortcut> {
    let key = match event.key() {
        Key::Tab => "Tab".to_string(),
        Key::Character(text) => text,
        _ => return None,
    };
    let modifiers = event.modifiers();
    shortcut_for(&key, modifiers.ctrl(), modifiers.meta(), modifiers.shift(), cfg!(target_os = "macos"))
}
```

- [ ] **Step 3: Exports**

In `crates/forge-features/src/lib.rs` add `mod settings;` and
`pub use settings::{use_settings, use_settings_provider, Settings, SettingsSection, SettingsStatus};`.

- [ ] **Step 4: New-session panel uses the workspace's folder and shows the saved model**

Replace the body of `NewSessionComposer` in `crates/forge-features/src/new_session.rs` (keep the imports, add `use forge_entities::model_label;` and `use crate::settings::use_settings;`):

```rust
#[component]
pub fn NewSessionComposer() -> Element {
    let workspace = use_workspace();
    let settings = use_settings();
    let mut mode = use_signal(|| "default".to_string());
    let mut text = use_signal(String::new);

    let projects = workspace.new_session_projects();
    let options: Vec<(String, String)> = projects
        .iter()
        .map(|project| {
            let path = project.path.to_string_lossy().to_string();
            (path.clone(), path)
        })
        .collect();
    // The chosen folder if it is still offered, otherwise the first one.
    let selected = workspace
        .new_folder()
        .map(|path| path.to_string_lossy().to_string())
        .filter(|path| options.iter().any(|(value, _)| value == path))
        .or_else(|| options.first().map(|(value, _)| value.clone()))
        .unwrap_or_default();
    let model = model_label(settings.model().as_deref());

    // `submit` is used by two handlers, so it may capture only `Copy` values (signals): it looks the
    // folder up again instead of capturing the `selected` string.
    let mut submit = move || {
        let body = text.read().trim().to_string();
        if body.is_empty() {
            return;
        }
        let projects = workspace.new_session_projects();
        let project = match workspace.new_folder() {
            Some(path) => projects
                .iter()
                .find(|project| project.path == path)
                .cloned()
                .or_else(|| projects.first().cloned()),
            None => projects.first().cloned(),
        };
        let Some(project) = project else {
            return;
        };
        workspace.start_new(project, parse_mode(&mode.read()), body);
        text.set(String::new());
    };
    let has_folder = !options.is_empty();

    rsx! {
        div { class: "new-session",
            if !has_folder {
                div { class: "banner info", "No existing project folder was found in your Claude Code history." }
            }
            div { class: "field",
                label { "Folder" }
                Select {
                    value: selected,
                    options,
                    disabled: !has_folder,
                    onchange: move |path: String| workspace.set_new_folder(Some(path.into())),
                }
            }
            div { class: "field",
                label { "Model" }
                span { class: "field-value", "{model}" }
                span { class: "hint", "Change it in Settings (⌘,)." }
            }
            div { class: "field",
                label { "Mode" }
                Select {
                    value: mode(),
                    options: mode_options(),
                    onchange: move |value| mode.set(value),
                }
            }
            TextArea {
                value: text(),
                placeholder: "What should Claude do?",
                disabled: !has_folder,
                oninput: move |value| text.set(value),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Enter && !event.modifiers().shift() && !event.is_composing() {
                        event.prevent_default();
                        submit();
                    }
                },
            }
            div { class: "composer-row",
                span { class: "spacer" }
                Button {
                    label: "Start",
                    kind: ButtonKind::Primary,
                    disabled: !has_folder,
                    onclick: move |_| submit(),
                }
            }
        }
    }
}
```

- [ ] **Step 5: Composition root**

In `crates/forge-app/src/main.rs`: add `SettingsUseCase` to the `forge_application` import and, in `build_services`, keep `config` and pass it on:

```rust
    let config = Arc::new(ConfigFileAdapter::new(config_path));
    let factory = Arc::new(SidecarProcessFactory::new(node, script, config.clone()));
    let settings = Arc::new(SettingsUseCase::new(config));
    ...
    WorkspaceServices { registry, projects, sessions, settings }
```

- [ ] **Step 6: Compile and run all tests**

The page does not call `use_settings_provider` yet, so the app would panic at runtime; add it now so the tree is consistent. In `crates/forge-pages/src/workspace_page.rs` change the top of `WorkspacePage` to:

```rust
    let settings = use_settings_provider();
    let workspace = use_workspace_provider();
```
(import `use_settings_provider` from `forge_features`) and the key handler to:

```rust
            onkeydown: move |event| {
                if !workspace.on_key(&event) {
                    settings.on_key(&event);
                }
            },
```

Run: `cargo test --workspace`
Expected: PASS, no new warnings (the `block v0.1.6` notice is fine).

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: settings state, saved-model start and composition wiring

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Theme rewrite and shared components

**Files:**
- Modify: `crates/forge-shared/src/theme.css`
- Modify: `crates/forge-shared/src/theme.rs`
- Modify: `crates/forge-shared/src/components.rs`
- Modify: `crates/forge-shared/src/lib.rs`

**Interfaces:**
- Produces (shared): `TextInput { value, placeholder, secret, oninput }`, `SettingRow { title, description, children }`; `StatusDot` renders a `!` for `Tone::Warning`.
- Produces (CSS class names used by later tasks): `.sidebar-group-header`, `.group-add`, `.sidebar-footer`, `.sidebar-footer-button`, `.settings-backdrop`, `.settings-panel`, `.settings-nav`, `.settings-nav-item`, `.settings-content`, `.settings-close`, `.setting-row`, `.setting-info`, `.setting-title`, `.setting-desc`, `.setting-control`, `.text-input`, `.hint`, `.field-value`, `.status-line`.

- [ ] **Step 1: Write the failing test**

In `crates/forge-shared/src/theme.rs` replace the class list in the first test with:

```rust
        for class in [
            ".workspace",
            ".sidebar",
            ".sidebar-item",
            ".sidebar-group-header",
            ".group-add",
            ".sidebar-footer",
            ".main-pane",
            ".messages",
            ".message",
            ".tool-call",
            ".permission-card",
            ".composer",
            ".status-dot",
            ".banner",
            ".settings-backdrop",
            ".settings-panel",
            ".settings-nav-item",
            ".setting-row",
            ".text-input",
        ] {
```

and add:

```rust
    #[test]
    fn the_running_marker_animates_but_not_for_people_who_asked_for_less_motion() {
        assert!(THEME_CSS.contains("@keyframes"));
        assert!(THEME_CSS.contains("prefers-reduced-motion"));
    }
```

Also change the doc comment above `THEME_CSS` to: "The app's stylesheet: a flat single-tone look after Claude Code desktop (spec §16.6)."

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p forge-shared`
Expected: FAIL (`theme.css is missing .sidebar-group-header`).

- [ ] **Step 3: Replace `theme.css`**

Overwrite `crates/forge-shared/src/theme.css` with:

```css
:root {
  --bg: #faf9f7;
  --bg-sidebar: #f3f1ee;
  --bg-card: #ffffff;
  --bg-hover: #ebe8e3;
  --bg-overlay: rgba(28, 25, 23, 0.4);
  --fg: #1c1917;
  --fg-muted: #8a837b;
  --border: #e6e2dc;
  --accent: #c2410c;
  --accent-soft: rgba(194, 65, 12, 0.25);
  --accent-fg: #ffffff;
  --warning: #b45309;
  --warning-fg: #ffffff;
  --danger: #b91c1c;
  --radius: 10px;
  --radius-lg: 14px;
  --shadow: 0 1px 2px rgba(28, 25, 23, 0.06), 0 4px 16px rgba(28, 25, 23, 0.06);
  --font: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  --mono: ui-monospace, "SF Mono", Menlo, monospace;
}

@media (prefers-color-scheme: dark) {
  :root {
    --bg: #1c1917;
    --bg-sidebar: #171412;
    --bg-card: #25211e;
    --bg-hover: #2a2622;
    --bg-overlay: rgba(0, 0, 0, 0.55);
    --fg: #f5f5f4;
    --fg-muted: #a39c94;
    --border: #302b27;
    --accent: #fb923c;
    --accent-soft: rgba(251, 146, 60, 0.3);
    --accent-fg: #1c1917;
    --warning: #facc15;
    --warning-fg: #1c1917;
    --danger: #f87171;
    --shadow: 0 1px 2px rgba(0, 0, 0, 0.4), 0 4px 16px rgba(0, 0, 0, 0.4);
  }
}

html, body, #main { height: 100%; margin: 0; }
body { font-family: var(--font); font-size: 14px; line-height: 1.5; background: var(--bg); color: var(--fg); }

.workspace { position: relative; display: flex; height: 100vh; outline: none; }

/* Sidebar */
.sidebar { width: 264px; flex: none; display: flex; flex-direction: column; overflow-y: auto; background: var(--bg-sidebar); border-right: 1px solid var(--border); padding: 12px 8px 0; box-sizing: border-box; }
.sidebar-new { margin: 0 4px 12px; }
.sidebar-new .button { width: 100%; }
.sidebar-group-header { display: flex; align-items: center; padding: 12px 8px 4px; }
.sidebar-group-title { flex: 1; min-width: 0; font-size: 12px; color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.sidebar-group-title.missing { text-decoration: line-through; }
.group-add { flex: none; width: 22px; height: 22px; border: none; border-radius: 6px; background: transparent; color: var(--fg-muted); font: inherit; font-size: 16px; line-height: 1; cursor: pointer; opacity: 0; }
.sidebar-group-header:hover .group-add, .group-add:focus-visible { opacity: 1; }
.group-add:hover { background: var(--bg-hover); color: var(--fg); }
.sidebar-item { display: flex; align-items: center; gap: 10px; padding: 6px 8px; border-radius: 8px; cursor: pointer; }
.sidebar-item:hover { background: var(--bg-hover); }
.sidebar-item.active { background: var(--bg-hover); font-weight: 600; }
.sidebar-item .title { flex: 1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.sidebar-item .subtitle { flex: none; font-size: 11px; color: var(--fg-muted); }
.sidebar-footer { position: sticky; bottom: 0; margin: auto -8px 0; padding: 8px; background: var(--bg-sidebar); border-top: 1px solid var(--border); }
.sidebar-footer-button { display: flex; align-items: center; gap: 8px; width: 100%; padding: 6px 8px; border: none; border-radius: 8px; background: transparent; color: var(--fg); font: inherit; cursor: pointer; text-align: left; }
.sidebar-footer-button:hover { background: var(--bg-hover); }
.sidebar-footer-button .keys { margin-left: auto; font-size: 12px; color: var(--fg-muted); }

/* Session markers: idle is a hollow ring, running a turning ring, needs-attention a filled "!" */
.status-dot { flex: none; width: 10px; height: 10px; border-radius: 50%; box-sizing: border-box; }
.status-dot.muted { border: 1.5px solid var(--fg-muted); opacity: 0.6; }
.status-dot.active { border: 2px solid var(--accent-soft); border-top-color: var(--accent); animation: spin 1s linear infinite; }
.status-dot.warning { width: 14px; height: 14px; display: inline-flex; align-items: center; justify-content: center; background: var(--warning); color: var(--warning-fg); font-size: 10px; font-weight: 700; line-height: 1; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) {
  .status-dot.active { animation: none; border-color: var(--accent); }
}

/* Conversation */
.main-pane { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.pane-header { padding: 12px 28px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.messages { flex: 1; overflow-y: auto; padding: 8px 28px 16px; }
.message { max-width: 760px; margin: 0 auto 18px; }
.message .role { font-size: 11px; color: var(--fg-muted); margin-bottom: 3px; }
.message .content { white-space: pre-wrap; overflow-wrap: anywhere; }
.message.user .content { display: inline-block; padding: 8px 12px; background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius); box-shadow: var(--shadow); }
.tool-call { display: block; width: fit-content; max-width: 100%; box-sizing: border-box; font-family: var(--mono); font-size: 12px; color: var(--fg-muted); margin-top: 6px; padding: 3px 9px; border: 1px solid var(--border); border-radius: 8px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.tool-call.failed { color: var(--danger); border-color: var(--danger); }

.permission-card { max-width: 760px; margin: 8px auto; padding: 12px 14px; background: var(--bg-card); border: 1px solid var(--warning); border-radius: var(--radius); box-shadow: var(--shadow); box-sizing: border-box; width: calc(100% - 56px); }
.permission-card .tool { font-weight: 600; }
.permission-card .input { font-family: var(--mono); font-size: 12px; color: var(--fg-muted); margin: 4px 0 10px; overflow-wrap: anywhere; }
.permission-actions { display: flex; gap: 8px; }

.banner { max-width: 760px; margin: 8px auto; padding: 8px 12px; border-radius: var(--radius); font-size: 13px; box-sizing: border-box; width: calc(100% - 56px); }
.sidebar .banner { width: auto; margin: 8px 4px; }
.banner.error { border: 1px solid var(--danger); color: var(--danger); }
.banner.info { border: 1px solid var(--border); color: var(--fg-muted); }

/* Composer: one rounded box */
.composer { display: flex; flex-direction: column; gap: 8px; width: calc(100% - 56px); max-width: 760px; margin: 0 auto 20px; padding: 10px 12px; box-sizing: border-box; background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius-lg); box-shadow: var(--shadow); }
.composer .textarea { border: none; background: transparent; padding: 2px; min-height: 48px; outline: none; }
.composer-row { display: flex; align-items: center; gap: 8px; }
.composer-row .spacer { flex: 1; }
.textarea { width: 100%; box-sizing: border-box; min-height: 64px; padding: 8px; resize: vertical; font: inherit; color: inherit; background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius); }
.select { font: inherit; color: inherit; background: var(--bg-card); border: 1px solid var(--border); border-radius: 8px; padding: 4px 8px; max-width: 100%; }
.text-input { font: inherit; color: inherit; background: var(--bg-card); border: 1px solid var(--border); border-radius: 8px; padding: 6px 10px; min-width: 280px; box-sizing: border-box; }
.text-input:focus, .textarea:focus, .select:focus { outline: 2px solid var(--accent-soft); outline-offset: 0; }

.button { font: inherit; color: inherit; background: var(--bg-card); border: 1px solid var(--border); border-radius: 8px; padding: 5px 14px; cursor: pointer; }
.button:hover:not(:disabled) { background: var(--bg-hover); }
.button:disabled { opacity: 0.5; cursor: default; }
.button.primary { background: var(--accent); color: var(--accent-fg); border-color: var(--accent); }
.button.primary:hover:not(:disabled) { background: var(--accent); filter: brightness(1.08); }
.button.danger { color: var(--danger); border-color: var(--danger); }

/* New-session panel */
.new-session { flex: 1; display: flex; flex-direction: column; justify-content: center; gap: 14px; padding: 24px; max-width: 720px; margin: 0 auto; width: 100%; box-sizing: border-box; }
.new-session .field { display: flex; align-items: center; gap: 12px; }
.new-session .field label { width: 64px; color: var(--fg-muted); }
.field-value { font-weight: 600; }
.hint { font-size: 12px; color: var(--fg-muted); }
.empty-state { margin: auto; color: var(--fg-muted); }

/* Settings overlay */
.settings-backdrop { position: absolute; inset: 0; z-index: 10; display: flex; align-items: center; justify-content: center; background: var(--bg-overlay); backdrop-filter: blur(3px); }
.settings-panel { position: relative; display: flex; width: min(960px, calc(100% - 64px)); height: min(640px, calc(100% - 64px)); background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius-lg); box-shadow: var(--shadow); overflow: hidden; }
.settings-nav { width: 220px; flex: none; padding: 20px 12px; background: var(--bg-sidebar); border-right: 1px solid var(--border); box-sizing: border-box; }
.settings-nav-heading { padding: 0 10px 8px; font-size: 12px; color: var(--fg-muted); }
.settings-nav-item { display: block; width: 100%; padding: 7px 10px; border: none; border-radius: 8px; background: transparent; color: var(--fg); font: inherit; text-align: left; cursor: pointer; }
.settings-nav-item:hover { background: var(--bg-hover); }
.settings-nav-item.active { background: var(--bg-hover); font-weight: 600; }
.settings-content { flex: 1; min-width: 0; overflow-y: auto; padding: 28px 36px; }
.settings-content h2 { margin: 0 0 8px; font-size: 16px; }
.settings-close { position: absolute; top: 14px; right: 14px; width: 28px; height: 28px; border: none; border-radius: 8px; background: transparent; color: var(--fg-muted); font-size: 16px; cursor: pointer; }
.settings-close:hover { background: var(--bg-hover); color: var(--fg); }
.setting-row { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 14px 0; border-bottom: 1px solid var(--border); }
.setting-info { min-width: 0; }
.setting-title { font-weight: 500; }
.setting-desc { font-size: 12px; color: var(--fg-muted); }
.setting-control { display: flex; align-items: center; gap: 8px; flex: none; }
.status-line { min-height: 20px; margin-top: 12px; font-size: 13px; color: var(--fg-muted); }
.status-line.error { color: var(--danger); }
```

- [ ] **Step 4: Add `TextInput`, `SettingRow` and the `!` glyph**

Append to `crates/forge-shared/src/components.rs`:

```rust
/// A one-line text field; `secret` hides the characters (an API key).
#[component]
pub fn TextInput(
    value: String,
    #[props(default)] placeholder: String,
    #[props(default)] secret: bool,
    oninput: EventHandler<String>,
) -> Element {
    rsx! {
        input {
            class: "text-input",
            r#type: if secret { "password" } else { "text" },
            value: "{value}",
            placeholder: "{placeholder}",
            autocomplete: "off",
            spellcheck: "false",
            oninput: move |evt| oninput.call(evt.value()),
        }
    }
}

/// One settings row: a title and a line of explanation on the left, the control (`children`) on the right.
#[component]
pub fn SettingRow(title: String, #[props(default)] description: String, children: Element) -> Element {
    rsx! {
        div { class: "setting-row",
            div { class: "setting-info",
                div { class: "setting-title", "{title}" }
                if !description.is_empty() {
                    div { class: "setting-desc", "{description}" }
                }
            }
            div { class: "setting-control", {children} }
        }
    }
}
```

Replace `StatusDot`:

```rust
#[component]
pub fn StatusDot(tone: Tone) -> Element {
    rsx! {
        span { class: tone.class(),
            if tone == Tone::Warning {
                "!"
            }
        }
    }
}
```

In `lib.rs`: `pub use components::{Button, ButtonKind, SettingRow, Select, StatusDot, TextArea, TextInput, Tone};`.

- [ ] **Step 5: Run to verify pass**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: flat theme, distinguishable markers, text input and setting row

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Sidebar: group `+` and the Settings entry

**Files:**
- Modify: `crates/forge-widgets/src/sidebar.rs`

**Interfaces:**
- Consumes: `Workspace::show_new_session_in(PathBuf)` (Task 4), `Settings::open()` (Task 4), CSS classes from Task 5.

- [ ] **Step 1: Edit `Sidebar`**

Import `use_settings` with the other `forge_features` items (`use forge_features::{use_settings, use_workspace, Active, EntryKey};`) and add `let settings = use_settings();` after `let workspace = use_workspace();`.

Replace the group title `div { class: if group.project.exists ... }` block with a header row:

```rust
                    div { class: "sidebar-group-header",
                        div {
                            class: if group.project.exists { "sidebar-group-title" } else { "sidebar-group-title missing" },
                            title: "{group.project.path.display()}",
                            "{project_label(&group.project.path)}"
                        }
                        if group.project.exists {
                            button {
                                class: "group-add",
                                title: "New session in this folder",
                                onclick: {
                                    let path = group.project.path.clone();
                                    move |_| workspace.show_new_session_in(path.clone())
                                },
                                "+"
                            }
                        }
                    }
```

Add, as the last child of the outer `div { class: "sidebar", ... }` (after the `for group in groups` loop):

```rust
            div { class: "sidebar-footer",
                button { class: "sidebar-footer-button", onclick: move |_| settings.open(),
                    span { "⚙" }
                    span { "Settings" }
                    span { class: "keys", "⌘," }
                }
            }
```

(The `⌘,` label is shown on every platform for now; Windows is out of scope, spec §15.1.)

- [ ] **Step 2: Compile and test**

Run: `cargo test --workspace`
Expected: PASS (the `project_label` tests still pass).

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: per-project + button and a Settings entry in the sidebar

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Settings overlay widget and page

**Files:**
- Create: `crates/forge-widgets/src/settings_overlay.rs`
- Modify: `crates/forge-widgets/src/lib.rs`
- Modify: `crates/forge-pages/src/workspace_page.rs`
- Modify (if the build says a dependency is missing): `crates/forge-widgets/Cargo.toml`

**Interfaces:**
- Consumes: `Settings` API (Task 4), `model_options` / `model_value` / `model_from_value` (Task 3), `SettingRow`, `TextInput`, `Select`, `Button` (Task 5).
- Produces: `SettingsOverlay {}` component (renders nothing while closed).

- [ ] **Step 1: Create the widget**

`crates/forge-widgets/src/settings_overlay.rs`:

```rust
use dioxus::prelude::*;
use forge_entities::{model_from_value, model_options, model_value};
use forge_features::{use_settings, SettingsSection, SettingsStatus};
use forge_shared::{Button, ButtonKind, Select, SettingRow, TextInput};

/// The settings overlay (spec §16): a left navigation and rows on the right. Renders nothing while
/// closed. Esc and ⌘, are handled by the page's key handler.
#[component]
pub fn SettingsOverlay() -> Element {
    let settings = use_settings();
    if !settings.is_open() {
        return rsx! {};
    }
    let section = settings.section();

    rsx! {
        div {
            class: "settings-backdrop",
            onclick: move |_| settings.close(),
            div {
                class: "settings-panel",
                // A click inside the panel must not reach the backdrop and close it.
                onclick: move |event| event.stop_propagation(),
                button { class: "settings-close", title: "Close (Esc)", onclick: move |_| settings.close(), "✕" }
                div { class: "settings-nav",
                    div { class: "settings-nav-heading", "Settings" }
                    button {
                        class: if section == SettingsSection::Connection { "settings-nav-item active" } else { "settings-nav-item" },
                        onclick: move |_| settings.set_section(SettingsSection::Connection),
                        "Connection"
                    }
                    button {
                        class: if section == SettingsSection::Model { "settings-nav-item active" } else { "settings-nav-item" },
                        onclick: move |_| settings.set_section(SettingsSection::Model),
                        "Model"
                    }
                }
                div { class: "settings-content",
                    match section {
                        SettingsSection::Connection => rsx! { ConnectionSection {} },
                        SettingsSection::Model => rsx! { ModelSection {} },
                    }
                    StatusLine {}
                }
            }
        }
    }
}

#[component]
fn ConnectionSection() -> Element {
    let settings = use_settings();
    let key_label = if settings.key_visible() { "Hide" } else { "Show" };
    rsx! {
        h2 { "Connection" }
        SettingRow { title: "Base URL", description: "Leave empty for https://api.anthropic.com.",
            TextInput {
                value: settings.base_url(),
                placeholder: "https://api.anthropic.com",
                oninput: move |value| settings.set_base_url(value),
            }
        }
        SettingRow { title: "API key", description: "Stored in Forge's own config file, never in ~/.claude.",
            TextInput {
                value: settings.api_key(),
                placeholder: "sk-ant-…",
                secret: !settings.key_visible(),
                oninput: move |value| settings.set_api_key(value),
            }
            Button { label: key_label, onclick: move |_| settings.toggle_key_visible() }
        }
        div { class: "hint", style: "margin-top: 12px;",
            "Sessions that are already open keep the old values until they are started again."
        }
        div { style: "margin-top: 16px;",
            Button {
                label: "Save",
                kind: ButtonKind::Primary,
                disabled: !settings.connection_dirty(),
                onclick: move |_| settings.save_connection(),
            }
        }
    }
}

#[component]
fn ModelSection() -> Element {
    let settings = use_settings();
    let mut custom = use_signal(String::new);
    let saved = settings.model();
    let options = model_options(saved.as_deref());
    let selected = model_value(saved.as_deref());
    let custom_empty = custom.read().trim().is_empty();
    rsx! {
        h2 { "Model" }
        SettingRow { title: "Default model", description: "New sessions start with this model. A choice is saved at once.",
            Select {
                value: selected,
                options,
                onchange: move |value: String| settings.choose_model(model_from_value(&value)),
            }
        }
        SettingRow { title: "Other model ID", description: "Use an ID that is not in the list.",
            TextInput {
                value: custom(),
                placeholder: "model-id",
                oninput: move |value| custom.set(value),
            }
            Button {
                label: "Use",
                disabled: custom_empty,
                onclick: move |_| {
                    settings.choose_model(custom.read().clone());
                    custom.set(String::new());
                },
            }
        }
        div { class: "hint", style: "margin-top: 12px;",
            "Sessions that are already open keep their model; a change applies to the next new session."
        }
    }
}

#[component]
fn StatusLine() -> Element {
    let settings = use_settings();
    match settings.status() {
        Some(SettingsStatus::Saved) => rsx! { div { class: "status-line", "Saved." } },
        Some(SettingsStatus::Error(message)) => rsx! { div { class: "status-line error", "{message}" } },
        None => rsx! { div { class: "status-line" } },
    }
}
```

- [ ] **Step 2: Register and render**

`crates/forge-widgets/src/lib.rs`: add `mod settings_overlay;` and `pub use settings_overlay::SettingsOverlay;`.

`crates/forge-pages/src/workspace_page.rs`: import `SettingsOverlay` (`use forge_widgets::{ChatWindow, NewSessionPanel, SettingsOverlay, Sidebar};`) and add `SettingsOverlay {}` as the last child of `div { class: "workspace", ... }`, after the `match`.

- [ ] **Step 3: Build and test**

Run: `cargo test --workspace`
Expected: PASS. If the build reports an unresolved crate in `forge-widgets` (`forge_entities`, `forge_shared`), add it to that crate's `Cargo.toml` `[dependencies]` as a `path = "../…"` entry like the other crates do.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: settings overlay with Connection and Model

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Manual verification in the real window, fixes, and the record

Plan 5 found two runtime bugs only in a live window (§15.11); budget for that here. Build the sidecar first if `sidecar/dist/index.js` is missing: `cd sidecar && npm run build`.

**Files:**
- Modify: `docs/superpowers/specs/2026-09-27-forge-design.md` (add §16.9 "As built")
- Modify: whatever the checklist shows to be broken

- [ ] **Step 1: Back up the real config**

Run: `cp "$HOME/Library/Application Support/Forge/config.json" "$TMPDIR/forge-config.backup.json" 2>/dev/null; echo done`
(The checklist edits the real file; the backup lets you restore it at the end.)

- [ ] **Step 2: Launch**

Run (from the repo root, in the background): `cargo run -p forge-app`
Expected: the window opens; the sidebar has a footer "⚙ Settings ⌘,".

- [ ] **Step 3: Walk the checklist and note each result**

1. Light and dark (toggle macOS appearance): both readable; no leftover white boxes in dark.
2. Markers: start a turn in a session (running = turning ring), trigger a Write permission (needs attention = filled `!`), idle = hollow ring. All three distinguishable at a glance.
3. `⌘,` opens Settings; `Esc` and ✕ and a click on the dimmed backdrop close it; a click inside the panel does not.
4. Connection: type a wrong key, Save → "Saved."; start a new session → the turn fails with an auth error; an already-open older session still works. Put the right key back, Save, start a new session → it works.
5. Key field: masked; Show reveals; a key pasted with a trailing newline does not leave Save enabled after saving.
6. Model: choose Haiku 4.5 → "Saved."; new-session panel says "Haiku 4.5"; the new session answers as that model. Choose Default → panel says "Default model". Type a custom ID → it appears as "Custom: …" and stays selected after reopening Settings.
7. Hand-edit `config.json` to add `"enabled_skills": ["x"]`, then save a key in the UI: the list is still in the file.
8. Replace `config.json` with `{ not json`: opening Settings shows the error; Save writes a fresh file.
9. The `+` beside a project opens the new-session panel with that folder preselected; it appears on hover only.
10. New-session panel and chat still work end to end (send, stop, permission allow/deny, ⌘N, ⌘W, Ctrl+Tab).

- [ ] **Step 4: Fix anything that failed**

For each failure: write a test if the cause is logic (TDD), otherwise fix CSS/markup, re-run the affected checklist item, and commit as `fix: …` with the trailer.

- [ ] **Step 5: Restore the config and record**

Run: `cp "$TMPDIR/forge-config.backup.json" "$HOME/Library/Application Support/Forge/config.json" 2>/dev/null; cargo test --workspace 2>&1 | grep "test result"`

Append `### 16.9 As built` to the spec: date, test count, checklist result per item, runtime fixes, and differences from §16 (at least these known ones: the live-session composer has no folder chip or model line, only the new-session panel shows the model; `SettingsUseCase` is one struct with `load`/`saved_model`/`save_connection`/`save_model` instead of `load_settings`/`save_settings`; the Settings footer shows `⌘,` on every platform). Commit:

```bash
git add -A
git commit -m "docs: record Plan 6 as built

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
