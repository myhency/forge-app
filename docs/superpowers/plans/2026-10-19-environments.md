# Local environments (Plan 53, stage A) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A person can save a named environment (folders, a branch and worktree choice for the first folder, `.env` variables), pick it from a chip on the new-session screen (mouse or ⌥⌘1…9, ⌥⌘0 for a new one), and start a session that works in the first folder, may use the others, and runs with the variables.

**Architecture:** `Environment` and the `.env` parser are pure code in `forge-domain`; the list lives in `AppConfig.environments` and is saved through `SettingsUseCase::save_code_settings` like the worktree records. A new `EnvVars` value travels from the new-session screen through `LiveCommand::SetEnv` → `StartSessionParams.env` → the sidecar's `start_session.env` → the SDK's `env` option. Menu order, the dialog's draft and the start plan are pure functions in `forge-features` (tested without a UI); `EnvironmentChip` and `EnvironmentDialog` in `forge-widgets` only draw them.

**Tech Stack:** Rust (Dioxus 0.7.10 desktop, workspace crates `forge-*`), TypeScript sidecar (vitest), the Claude Agent SDK.

**Spec:** `docs/superpowers/specs/2026-10-19-environments-design.md` (evidence: `docs/superpowers/roadmap/observations.md`, Round 21). Read both before starting.

## Global Constraints

- Local only. No cloud, SSH, Slack or Remote Control tab, submenu, switch or settings row, not even disabled. No sidebar `환경` filter. No side-panel card or `시작 위치` line (stage B).
- Every new user-visible string goes through `tr!` / `msg!` and gets a Korean entry in `crates/forge-shared/src/ko.rs` (use `python3 scripts/i18n_add.py 'English' 'Korean' …` from the repo root; it keeps the file sorted). Korean text of the desktop app (kept as written in the observations) is the model.
- Variables are stored as plain text in `config.json`. The dialog must say "Saved in Forge's settings file on this computer." and must not say "securely" or "encrypted".
- Variable values must never appear in any `Debug` output, log line or error message. Carry them only as `forge_domain::EnvVars` (its `Debug` prints names only).
- Menu keys are matched by the physical key (`Digit1`…, Korean-input-method safe): ⌥⌘1…⌥⌘9 pick row 1…9 (row 1 is `이 컴퓨터`), ⌥⌘0 opens `새 환경`. Active on the new-session screen only.
- Menu limit: at most 9 numbered rows (`이 컴퓨터` + 8 saved). Order: `이 컴퓨터`, pinned environments, the most recent unpinned one; the rest under `더 보기`; each group by `last_used_at` descending.
- A saved environment holds: id, name, folders (first = working folder), `branch` and `worktree` (of the first folder only), `env_vars` text, `pinned`, `created_at`, `last_used_at` (epoch ms). It does not hold model, effort or mode.
- Worktree rule at start: only when `worktree` is on, and only if the first folder is a git repository (checked at send time with `WorktreesUseCase::branches`); a non-repository starts in the folder itself.
- Trust: a folder added to the dialog that is not on the trusted list shows the existing question (`Allow Claude to use this folder and its subfolders?`); cancelling does not add the folder.
- Rust code style of the repo: comments only where the reason is not obvious; tests next to the code in `#[cfg(test)] mod tests`; test names are sentences.
- Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Review Focus

- Variable text written on Windows (CRLF line endings, a trailing newline, a UTF-8 BOM, `KEY = value` with spaces around `=`): it must parse like the same text with `\n`. (Task 1 tests.)
- An environment whose first folder no longer exists, or whose variables no longer parse (hand-edited config): sending shows a notice, starts nothing and keeps the typed text. (Task 3 `start_plan` tests, Task 4 send path.)
- The same folder added twice, or an extra folder equal to the first: stored once; never passed as an additional directory of itself. (Task 1 `Environment::normalized` tests, Task 3 `start_plan` tests.)
- Deleting the environment that is picked on the open new-session screen (from the dialog): the chip falls back to `이 컴퓨터` and the folder chips return. (Task 4.)
- A config whose `environments` value is malformed: the rest of the config still loads and the menu shows only `이 컴퓨터`. (Task 1 test.)

---

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/forge-domain/src/environment.rs` (new) | `Environment`, `EnvVars`, `parse_env`, `EnvParseError`, `new_environment_id` |
| `crates/forge-domain/src/config.rs` | `AppConfig.environments` |
| `crates/forge-ports/src/sidecar_port.rs` | `StartSessionParams.env: EnvVars` |
| `crates/forge-adapter-sidecar/src/protocol.rs` | wire `env` object on `start_session` |
| `sidecar/src/protocol.ts`, `sidecar/src/session.ts`, `sidecar/test/session.test.ts` | pass `env` to the SDK |
| `crates/forge-application/src/live_session.rs`, `live_session_actor.rs` | `LiveCommand::SetEnv`, `handle.set_env`, actor state |
| `crates/forge-features/src/environment.rs` (new) | menu rows and numbering, dialog draft, start plan, key request type |
| `crates/forge-features/src/shortcut.rs` | `PickEnvironment(n)`, `NewEnvironment` bindings |
| `crates/forge-features/src/workspace.rs` | environment list and dialog signals, save/delete/pin, key request, `start_new` with an environment |
| `crates/forge-features/src/new_session.rs` | draft `environment` signal; send path with an environment |
| `crates/forge-widgets/src/environment_chip.rs` (new) | the chip and its menu |
| `crates/forge-widgets/src/environment_dialog.rs` (new) | `새 환경` / `환경 편집` dialog, variables page, confirmations |
| `crates/forge-widgets/src/new_session_controls.rs`, `new_session_panel.rs` | hide folder chips while an environment is picked; add the chip |
| `crates/forge-pages/src/workspace_page.rs` | mount `EnvironmentDialog` |
| `crates/forge-shared/src/theme.css`, `ko.rs` | styles and Korean text |
| `docs/superpowers/roadmap/backlog.md` | row OTH-11 |

---

### Task 1: Domain — `Environment`, `EnvVars`, `.env` parser, config field

**Files:**
- Create: `crates/forge-domain/src/environment.rs`
- Modify: `crates/forge-domain/src/lib.rs` (add `mod environment;` and the `pub use` line)
- Modify: `crates/forge-domain/src/config.rs` (field, default, tests)

**Interfaces:**
- Produces (used by every later task):
  - `pub struct Environment { pub id: String, pub name: String, pub folders: Vec<PathBuf>, pub branch: Option<String>, pub worktree: bool, pub env_vars: String, pub pinned: bool, pub created_at: i64, pub last_used_at: i64 }` (`Debug, Clone, PartialEq, Eq, Serialize, Deserialize`; every field but `id` and `name` has `#[serde(default)]`)
  - `Environment::normalized(self) -> Environment` — trims the name, drops duplicate folders (first spelling wins), clears `branch`/`worktree` when there is no folder.
  - `pub fn new_environment_id(seed: u64) -> String` — `env-<16 hex digits>`.
  - `pub struct EnvVars(pub Vec<(String, String)>)` (`Clone, PartialEq, Eq, Default`; `Debug` prints names only).
  - `pub fn parse_env(text: &str) -> Result<Vec<(String, String)>, EnvParseError>`; `pub struct EnvParseError { pub line: usize, pub reason: EnvParseReason }`; `pub enum EnvParseReason { NotAPair, BadKey, UnclosedQuote }` (`Debug, Clone, Copy, PartialEq, Eq`).
  - `AppConfig.environments: Vec<Environment>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-domain/src/environment.rs` with only the tests first (the types come in Step 3). Put this at the bottom of the file you will finish in Step 3; for now create the file with this content and `use super::*;` failing to resolve:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn env(text: &str) -> Vec<(String, String)> {
        parse_env(text).unwrap()
    }

    fn pair(key: &str, value: &str) -> (String, String) {
        (key.to_string(), value.to_string())
    }

    #[test]
    fn plain_pairs_comments_blank_lines_and_export_are_read() {
        let text = "# a comment\n\nA=1\nexport B=two words\nC=\n";
        assert_eq!(env(text), vec![pair("A", "1"), pair("B", "two words"), pair("C", "")]);
    }

    #[test]
    fn a_quoted_value_keeps_its_spaces_and_may_run_over_lines() {
        assert_eq!(env("D=\"  x y \""), vec![pair("D", "  x y ")]);
        assert_eq!(env("CONFIG=\"key1=val1\nkey2=val2\"\nE='it'"), vec![pair("CONFIG", "key1=val1\nkey2=val2"), pair("E", "it")]);
    }

    #[test]
    fn an_equals_sign_inside_a_value_and_a_trailing_comment_are_handled() {
        assert_eq!(env("URL=https://x.test/?a=b # note"), vec![pair("URL", "https://x.test/?a=b")]);
    }

    #[test]
    fn windows_line_endings_a_bom_and_spaces_around_the_equals_sign_read_like_unix_text() {
        let unix = env("A=1\nB=\"x\ny\"\nC = 3");
        let windows = env("\u{feff}A=1\r\nB=\"x\r\ny\"\r\nC = 3\r\n");
        assert_eq!(unix, vec![pair("A", "1"), pair("B", "x\ny"), pair("C", "3")]);
        assert_eq!(windows, unix);
    }

    #[test]
    fn a_later_duplicate_wins() {
        assert_eq!(env("A=1\nB=x\nA=2"), vec![pair("B", "x"), pair("A", "2")]);
    }

    #[test]
    fn errors_name_the_line_and_the_reason() {
        assert_eq!(parse_env("A=1\nnot a pair").unwrap_err(), EnvParseError { line: 2, reason: EnvParseReason::NotAPair });
        assert_eq!(parse_env("1A=x").unwrap_err(), EnvParseError { line: 1, reason: EnvParseReason::BadKey });
        assert_eq!(parse_env("A=1\nB=\"open\nstill open").unwrap_err(), EnvParseError { line: 2, reason: EnvParseReason::UnclosedQuote });
    }

    #[test]
    fn env_vars_debug_shows_names_and_never_values() {
        let vars = EnvVars(vec![pair("TOKEN", "s3cret"), pair("NODE_ENV", "production")]);
        let shown = format!("{vars:?}");
        assert!(shown.contains("TOKEN") && shown.contains("NODE_ENV"));
        assert!(!shown.contains("s3cret") && !shown.contains("production"));
    }

    #[test]
    fn normalized_trims_the_name_drops_repeated_folders_and_clears_the_first_folder_choices_when_there_is_none() {
        let mut environment = Environment {
            id: "env-1".into(),
            name: "  work  ".into(),
            folders: vec!["/a".into(), "/b".into(), "/a".into()],
            branch: Some("dev".into()),
            worktree: true,
            env_vars: String::new(),
            pinned: false,
            created_at: 1,
            last_used_at: 2,
        };
        let kept = environment.clone().normalized();
        assert_eq!(kept.name, "work");
        assert_eq!(kept.folders, vec![std::path::PathBuf::from("/a"), "/b".into()]);
        assert_eq!(kept.branch.as_deref(), Some("dev"));
        environment.folders.clear();
        let empty = environment.normalized();
        assert_eq!((empty.branch, empty.worktree), (None, false));
    }

    #[test]
    fn an_id_is_sixteen_hex_digits_after_the_prefix() {
        assert_eq!(new_environment_id(255), "env-00000000000000ff");
    }
}
```

In `crates/forge-domain/src/config.rs`, inside its existing `#[cfg(test)] mod tests` (the module that holds `a_config_from_an_older_forge_loads_with_no_worktrees_and_no_auto_archive`), add:

```rust
    #[test]
    fn a_config_from_an_older_forge_has_no_environments_and_a_broken_value_reads_as_none() {
        let old: AppConfig = serde_json::from_str(r#"{"base_url":null,"api_key":null}"#).unwrap();
        assert!(old.environments.is_empty());
        let broken: AppConfig = serde_json::from_str(r#"{"base_url":"https://x.test","api_key":null,"environments":"oops"}"#).unwrap();
        assert_eq!(broken.base_url.as_deref(), Some("https://x.test"));
        assert!(broken.environments.is_empty());
    }

    #[test]
    fn environments_are_saved_and_read_back() {
        let config = AppConfig {
            environments: vec![crate::Environment {
                id: "env-1".into(),
                name: "work".into(),
                folders: vec!["/a".into(), "/b".into()],
                branch: Some("dev".into()),
                worktree: true,
                env_vars: "A=1".into(),
                pinned: true,
                created_at: 5,
                last_used_at: 9,
            }],
            ..AppConfig::default()
        };
        let text = serde_json::to_string(&config).unwrap();
        assert_eq!(serde_json::from_str::<AppConfig>(&text).unwrap(), config);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p forge-domain environment 2>&1 | tail -20`
Expected: FAIL to compile (`cannot find ... Environment`, `parse_env`, `EnvVars`).

- [ ] **Step 3: Write the implementation**

Put this above the `#[cfg(test)]` block in `crates/forge-domain/src/environment.rs`:

```rust
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A saved set of folders and variables a new session can start from. `folders[0]` is the working
/// folder; the others are added directories. `branch` and `worktree` belong to the first folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub folders: Vec<PathBuf>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub worktree: bool,
    /// `.env` text, as typed.
    #[serde(default)]
    pub env_vars: String,
    #[serde(default)]
    pub pinned: bool,
    /// Epoch milliseconds.
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub last_used_at: i64,
}

impl Environment {
    /// The environment as it is stored: trimmed name, each folder once (the first spelling wins), and
    /// no branch or worktree choice when there is no folder to apply it to.
    pub fn normalized(mut self) -> Environment {
        self.name = self.name.trim().to_string();
        let mut seen: Vec<PathBuf> = Vec::new();
        for folder in std::mem::take(&mut self.folders) {
            if !seen.contains(&folder) {
                seen.push(folder);
            }
        }
        self.folders = seen;
        if self.folders.is_empty() {
            self.branch = None;
            self.worktree = false;
        }
        self
    }
}

/// `seed` is any number that differs from call to call (the clock's nanoseconds).
pub fn new_environment_id(seed: u64) -> String {
    format!("env-{seed:016x}")
}

/// Variables for a session's process. Their values may be secrets, so `Debug` shows names only.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct EnvVars(pub Vec<(String, String)>);

impl std::fmt::Debug for EnvVars {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.iter().map(|(name, _)| name)).finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvParseReason {
    NotAPair,
    BadKey,
    UnclosedQuote,
}

/// A line of the variables text that is not valid; `line` counts from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvParseError {
    pub line: usize,
    pub reason: EnvParseReason,
}

fn valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Reads `.env` text: `KEY=value` lines, `#` comment lines, blank lines, an optional `export `,
/// a value in `"` or `'` that may run over several lines, a trailing ` #` comment on an unquoted
/// value. A later duplicate replaces the earlier one.
pub fn parse_env(text: &str) -> Result<Vec<(String, String)>, EnvParseError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<(String, String)> = Vec::new();
    let mut next = 0;
    while next < lines.len() {
        let number = next + 1;
        let line = lines[next].trim();
        next += 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").map_or(line, str::trim_start);
        let fail = |reason| EnvParseError { line: number, reason };
        let Some((key, rest)) = line.split_once('=') else {
            return Err(fail(EnvParseReason::NotAPair));
        };
        let key = key.trim();
        if !valid_key(key) {
            return Err(fail(EnvParseReason::BadKey));
        }
        let rest = rest.trim_start();
        let value = match rest.chars().next() {
            Some(quote @ ('"' | '\'')) => {
                let mut value = String::new();
                let mut tail = &rest[1..];
                loop {
                    if let Some(end) = tail.find(quote) {
                        value.push_str(&tail[..end]);
                        break;
                    }
                    value.push_str(tail);
                    if next >= lines.len() {
                        return Err(fail(EnvParseReason::UnclosedQuote));
                    }
                    value.push('\n');
                    tail = lines[next];
                    next += 1;
                }
                value
            }
            _ => rest.split(" #").next().unwrap_or("").trim_end().to_string(),
        };
        out.retain(|(existing, _)| existing != key);
        out.push((key.to_string(), value));
    }
    Ok(out)
}
```

In `crates/forge-domain/src/lib.rs` add `mod environment;` with the other `mod` lines (alphabetical neighbours `effort`, `extension`) and, with the `pub use` lines:

```rust
pub use environment::{new_environment_id, parse_env, EnvParseError, EnvParseReason, EnvVars, Environment};
```

In `crates/forge-domain/src/config.rs`, in `pub struct AppConfig` after `auto_archive_days`:

```rust
    /// Saved environments of the new-session screen.
    #[serde(default, deserialize_with = "lenient")]
    pub environments: Vec<crate::Environment>,
```

and in `impl Default for AppConfig`, after `auto_archive_days: None,`:

```rust
            environments: Vec::new(),
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p forge-domain 2>&1 | tail -8`
Expected: PASS (all of forge-domain, including the two new config tests and the nine environment tests). Fix any compile error from a struct literal of `AppConfig` elsewhere in the workspace with `cargo build --workspace --tests 2>&1 | grep -B2 -A8 "missing field"` (none are expected: other code uses `..AppConfig::default()`).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-domain
git commit -m "feat: Environment, EnvVars and the .env parser in the domain

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Start path — variables from the live session to the SDK

**Files:**
- Modify: `crates/forge-ports/src/sidecar_port.rs` (field)
- Modify: `crates/forge-adapter-sidecar/src/protocol.rs` (wire field, test)
- Modify: `sidecar/src/protocol.ts`, `sidecar/src/session.ts`, `sidecar/test/session.test.ts`
- Modify: `crates/forge-application/src/live_session.rs`, `live_session_actor.rs`
- Modify (mechanical, one line each): every `StartSessionParams { … }` literal

**Interfaces:**
- Consumes: `forge_domain::EnvVars` (Task 1).
- Produces:
  - `StartSessionParams.env: EnvVars`.
  - `LiveCommand::SetEnv(EnvVars)` and `LiveSessionHandle::set_env(&self, env: EnvVars)` (like `allow_tools`: only a session whose sidecar has not started, or has only warmed up, takes it; later calls are ignored).
  - Sidecar `start_session` command with optional `env: Record<string, string>`; the SDK gets `env: { ...process.env, ...command.env }` only when `env` is non-empty.

- [ ] **Step 1: Write the failing tests**

In `sidecar/test/session.test.ts`, next to the test `passes includePartialMessages: true to the SDK options` (it uses `toQueryHandle`, `QueryFn`, `startCommand`; copy its shape), add:

```ts
  it("passes the command's env over the process environment, and none when there is none", async () => {
    const seen: any[] = [];
    const queryFn: QueryFn = (args) => {
      seen.push(args.options);
      return toQueryHandle((async function* () {})());
    };
    process.env.FORGE_TEST_KEEP = "kept";
    try {
      const withEnv = new SidecarSession(queryFn, () => {});
      await withEnv.handle({ ...startCommand, env: { FORGE_TEST_VAR: "v", FORGE_TEST_KEEP: "replaced" } });
      const without = new SidecarSession(queryFn, () => {});
      await without.handle({ ...startCommand, id: "2" });
    } finally {
      delete process.env.FORGE_TEST_KEEP;
    }
    expect(seen[0].env.FORGE_TEST_VAR).toBe("v");
    expect(seen[0].env.FORGE_TEST_KEEP).toBe("replaced");
    expect(seen[0].env.PATH).toBe(process.env.PATH);
    expect(seen[1].env).toBeUndefined();
  });
```

(`startCommand` is the constant at the top of `session.test.ts`; `await session.handle(startCommand)` is how `passes includePartialMessages: true to the SDK options` runs a start.)

In `crates/forge-adapter-sidecar/src/protocol.rs`, in its tests module after `start_session_carries_the_defaults_and_leaves_the_style_out_when_there_is_none`, add (and add `env: forge_domain::EnvVars::default(),` to `params_with`'s literal):

```rust
    #[test]
    fn start_session_sends_env_as_an_object_and_leaves_it_out_when_empty() {
        let none = serde_json::to_value(SidecarCommand::start_session("1".to_string(), params_with(Vec::new()))).unwrap();
        assert!(none.get("env").is_none());
        let mut params = params_with(Vec::new());
        params.env = forge_domain::EnvVars(vec![("A".to_string(), "1".to_string()), ("B".to_string(), "x y".to_string())]);
        let some = serde_json::to_value(SidecarCommand::start_session("1".to_string(), params)).unwrap();
        assert_eq!(some["env"], serde_json::json!({ "A": "1", "B": "x y" }));
    }
```

In `crates/forge-application/src/live_session.rs` tests, after `allowed_tools_go_to_the_first_start`, add (and add `env: forge_domain::EnvVars::default(),` to `start_params`):

```rust
    #[tokio::test]
    async fn env_goes_to_the_first_start() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.set_env(forge_domain::EnvVars(vec![("A".to_string(), "1".to_string())]));
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        let mut expected = start_params(None);
        expected.env = forge_domain::EnvVars(vec![("A".to_string(), "1".to_string())]);
        assert_eq!(control.calls(), vec![Call::Start(expected), Call::Send("hi".to_string())]);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `(cd sidecar && npx vitest run test/session.test.ts -t "command's env") 2>&1 | tail -15` — Expected: FAIL (`env` is not passed).
Run: `cargo test -p forge-adapter-sidecar -p forge-application env 2>&1 | tail -15` — Expected: FAIL to compile (`no field env`, `no method set_env`).

- [ ] **Step 3: Write the implementation**

`crates/forge-ports/src/sidecar_port.rs`: add to `StartSessionParams`, after `allowed_tools`:

```rust
    /// Variables the session's process runs with, over Forge's own environment. Values may be secrets.
    pub env: forge_domain::EnvVars,
```

`crates/forge-adapter-sidecar/src/protocol.rs`: in `SidecarCommand::StartSession` after `allowed_tools`:

```rust
        #[serde(skip_serializing_if = "BTreeMap::is_empty")]
        env: BTreeMap<String, String>,
```

and in `start_session(...)` after `allowed_tools: params.allowed_tools,`:

```rust
            env: params.env.0.into_iter().collect(),
```

`sidecar/src/protocol.ts`: in the `start_session` variant after `allowed_tools?: string[];` add `env?: Record<string, string>;`.

`sidecar/src/session.ts`: in the options object, after the line `allowedTools: command.allowed_tools ?? [],` add:

```ts
          // Variables of the environment the session starts from, over Forge's own (the SDK would
          // replace the whole environment with what it is given).
          ...(command.env && Object.keys(command.env).length > 0 ? { env: { ...process.env, ...command.env } } : {}),
```

`crates/forge-application/src/live_session.rs`: in `enum LiveCommand` after `AllowTools(Vec<String>),`:

```rust
    /// Variables the next start runs with.
    SetEnv(forge_domain::EnvVars),
```

and in `impl LiveSessionHandle` after `allow_tools`:

```rust
    /// Variables the session's process runs with (the SDK reads them once, at start). Like folders,
    /// only a session whose sidecar has not started takes them.
    pub fn set_env(&self, env: forge_domain::EnvVars) {
        let _ = self.commands.send(LiveCommand::SetEnv(env));
    }
```

`crates/forge-application/src/live_session_actor.rs`: field after `allowed_tools: Vec<String>,` (line ~34): `env: forge_domain::EnvVars,`; in the constructor next to `allowed_tools: Vec::new(),` (line ~70): `env: forge_domain::EnvVars::default(),`; in the command match after `LiveCommand::AllowTools(tools) => self.allow_tools(tools).await,`:

```rust
            LiveCommand::SetEnv(env) => self.set_env(env).await,
```

a method after `allow_tools`:

```rust
    async fn set_env(&mut self, env: forge_domain::EnvVars) {
        if self.sidecar.is_some() && !self.warm_only {
            return;
        }
        self.env = env;
        if self.warm_only {
            self.warm_only = false;
            self.discard_sidecar(true).await;
        }
    }
```

and in `start_sidecar`'s literal after `allowed_tools: self.allowed_tools.clone(),`: `env: self.env.clone(),`.

Every other `StartSessionParams { … }` literal needs `env: forge_domain::EnvVars::default(),` (or `EnvVars::default()` where it is imported). Find them with the compiler:

Run: `cargo build --workspace --tests --examples 2>&1 | grep -A6 "missing field \`env\`"`
Known sites: `crates/forge-ports/src/sidecar_port.rs` (~168), `crates/forge-adapter-sidecar/src/protocol.rs` (~188, ~303, ~343), `crates/forge-adapter-sidecar/tests/contract_test.rs`, `tests/factory_test.rs`, `examples/spike_drive_sidecar.rs`, `crates/forge-application/src/command_probe.rs` (~52, ~154), `live_session.rs` (`start_params`), `use_cases/session_lifecycle.rs` (~26, ~68), `registry_host.rs` (~355). Repeat until the build is clean.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `(cd sidecar && npm run build && npx vitest run) 2>&1 | tail -8` — Expected: PASS.
Run: `cargo test -p forge-ports -p forge-adapter-sidecar -p forge-application 2>&1 | tail -8` — Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A crates sidecar/src sidecar/test
git commit -m "feat: a session starts with the variables of its environment

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Features — menu order, dialog draft, start plan, workspace API, shortcuts

**Files:**
- Create: `crates/forge-features/src/environment.rs`
- Modify: `crates/forge-features/src/lib.rs` (mod + exports)
- Modify: `crates/forge-features/src/shortcut.rs`
- Modify: `crates/forge-features/src/workspace.rs`
- Modify: `crates/forge-features/src/new_session.rs`

**Interfaces:**
- Consumes: Task 1 types; `handle.set_env`, `handle.add_directories` (Task 2 / existing).
- Produces (used by Tasks 4 and 5):
  - `pub struct EnvRow { pub id: Option<String>, pub name: String, pub number: Option<usize>, pub pinned: bool }` (`id == None` is `이 컴퓨터`; its `name` is the label the caller passed).
  - `pub struct EnvironmentMenu { pub visible: Vec<EnvRow>, pub more: Vec<EnvRow> }` and `pub fn environment_menu(environments: &[Environment], query: &str, this_computer: &str) -> EnvironmentMenu` (with a non-empty query: all matches in `visible`, no numbers, `more` empty).
  - `pub fn numbered_ids(environments: &[Environment]) -> Vec<Option<String>>` — row 1 = `None` (this computer), then the visible saved ones; at most 9.
  - `pub struct EnvironmentDraft { pub id: Option<String>, pub name: String, pub folders: Vec<PathBuf>, pub branch: Option<String>, pub worktree: bool, pub env_vars: String }` with `EnvironmentDraft::blank(name: String, folder: Option<PathBuf>)`, `EnvironmentDraft::of(&Environment)`, `fn changed_from(&self, initial: &EnvironmentDraft) -> bool`, `fn can_save(&self) -> bool`, `fn variables_error(&self) -> Option<EnvParseError>`, `fn build(&self, previous: Option<&Environment>, id_seed: u64, now: i64) -> Environment`.
  - `pub struct EnvironmentStart { pub folder: PathBuf, pub extra: Vec<PathBuf>, pub env: EnvVars, pub worktree: Option<NewWorktree> }`, `pub enum StartError { NoFolder, Variables(EnvParseError) }`, `pub fn start_plan(environment: &Environment) -> Result<EnvironmentStart, StartError>`.
  - `pub enum EnvironmentKey { Pick(usize), New }`, `pub enum EnvironmentRequest { New { folder: Option<PathBuf> }, Edit(String) }`.
  - `Shortcut::PickEnvironment(usize)`, `Shortcut::NewEnvironment`.
  - On `Workspace`: `environments() -> Vec<Environment>`; `save_environment(Environment)`; `delete_environment(String)`; `set_environment_pinned(String, bool)`; `environment_key() -> Option<EnvironmentKey>`, `clear_environment_key()`; `environment_request() -> Option<EnvironmentRequest>`, `open_environment_dialog(EnvironmentRequest)`, `close_environment_dialog()`; `async folder_branches(PathBuf) -> Option<forge_ports::BranchList>`; `async pick_folder() -> Option<PathBuf>`; `async needs_trust(PathBuf) -> bool`; `trust_folder(PathBuf)`; and `start_new(...)` gaining a last parameter `environment: Option<EnvironmentStart>`.
  - `NewSessionDraft.environment: Signal<Option<String>>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-features/src/environment.rs` containing the tests below (the implementation follows in Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn env(id: &str, name: &str, pinned: bool, last_used: i64) -> Environment {
        Environment {
            id: id.to_string(),
            name: name.to_string(),
            folders: vec![format!("/work/{name}").into()],
            branch: None,
            worktree: false,
            env_vars: String::new(),
            pinned,
            created_at: 0,
            last_used_at: last_used,
        }
    }

    fn names(rows: &[EnvRow]) -> Vec<String> {
        rows.iter().map(|row| row.name.clone()).collect()
    }

    #[test]
    fn the_menu_lists_this_computer_pinned_ones_and_the_newest_one_with_the_rest_under_more() {
        let list = vec![env("a", "old", false, 1), env("b", "new", false, 9), env("c", "mid", false, 5), env("d", "pin", true, 0)];
        let menu = environment_menu(&list, "", "This computer");
        assert_eq!(names(&menu.visible), vec!["This computer", "pin", "new"]);
        assert_eq!(names(&menu.more), vec!["mid", "old"]);
        assert_eq!(menu.visible.iter().map(|row| row.number).collect::<Vec<_>>(), vec![Some(1), Some(2), Some(3)]);
        assert_eq!(menu.more.iter().map(|row| row.number).collect::<Vec<_>>(), vec![None, None]);
        assert_eq!(menu.visible[0].id, None);
    }

    #[test]
    fn with_no_saved_environment_only_this_computer_shows() {
        let menu = environment_menu(&[], "", "This computer");
        assert_eq!(names(&menu.visible), vec!["This computer"]);
        assert!(menu.more.is_empty());
    }

    #[test]
    fn at_most_nine_rows_are_numbered_and_extra_pinned_ones_go_under_more() {
        let list: Vec<Environment> = (0..12).map(|n| env(&format!("e{n}"), &format!("p{n:02}"), true, n)).collect();
        let menu = environment_menu(&list, "", "This computer");
        assert_eq!(menu.visible.len(), 9);
        assert_eq!(menu.more.len(), 4);
        assert_eq!(numbered_ids(&list).len(), 9);
        assert_eq!(numbered_ids(&list)[0], None);
    }

    #[test]
    fn a_query_lists_every_match_flat_without_numbers_and_ignores_case() {
        let list = vec![env("a", "Work App", false, 1), env("b", "play", false, 2), env("c", "work docs", false, 3)];
        let menu = environment_menu(&list, "WORK", "This computer");
        assert_eq!(names(&menu.visible), vec!["work docs", "Work App"]);
        assert!(menu.visible.iter().all(|row| row.number.is_none()) && menu.more.is_empty());
    }

    #[test]
    fn numbered_ids_follow_the_menu_so_a_key_picks_the_row_it_shows() {
        let list = vec![env("a", "old", false, 1), env("b", "new", false, 9)];
        assert_eq!(numbered_ids(&list), vec![None, Some("b".to_string())]);
    }

    #[test]
    fn a_blank_draft_takes_the_folder_and_is_not_changed_until_edited() {
        let draft = EnvironmentDraft::blank("app".to_string(), Some("/work/app".into()));
        assert_eq!(draft.folders, vec![std::path::PathBuf::from("/work/app")]);
        assert!(draft.worktree);
        assert!(!draft.changed_from(&draft.clone()));
        let mut edited = draft.clone();
        edited.name = "other".to_string();
        assert!(edited.changed_from(&draft));
    }

    #[test]
    fn a_draft_can_be_saved_with_a_name_and_valid_variables_and_reports_the_bad_line() {
        let mut draft = EnvironmentDraft::blank("app".to_string(), None);
        assert!(draft.can_save());
        draft.name = "   ".to_string();
        assert!(!draft.can_save());
        draft.name = "app".to_string();
        draft.env_vars = "A=1\nnope".to_string();
        assert!(!draft.can_save());
        assert_eq!(draft.variables_error().map(|error| error.line), Some(2));
    }

    #[test]
    fn building_a_new_environment_sets_the_ids_and_times_and_an_edit_keeps_what_the_dialog_does_not_show() {
        let mut draft = EnvironmentDraft::blank(" app ".to_string(), Some("/a".into()));
        draft.folders.push("/a".into());
        let made = draft.build(None, 255, 1000);
        assert_eq!((made.id.as_str(), made.name.as_str(), made.created_at, made.last_used_at), ("env-00000000000000ff", "app", 1000, 1000));
        assert_eq!(made.folders, vec![std::path::PathBuf::from("/a")]);

        let mut stored = made.clone();
        stored.pinned = true;
        stored.last_used_at = 5000;
        let mut edit = EnvironmentDraft::of(&stored);
        edit.name = "renamed".to_string();
        let changed = edit.build(Some(&stored), 1, 9999);
        assert_eq!((changed.id.as_str(), changed.name.as_str(), changed.pinned, changed.created_at, changed.last_used_at), (stored.id.as_str(), "renamed", true, 1000, 5000));
    }

    #[test]
    fn the_start_plan_uses_the_first_folder_the_others_as_extras_and_the_variables() {
        let mut environment = env("a", "app", false, 0);
        environment.folders = vec!["/a".into(), "/b".into(), "/a".into(), "/c".into()];
        environment.env_vars = "X=1\nY=\"two words\"".to_string();
        environment.worktree = true;
        environment.branch = Some("dev".to_string());
        let plan = start_plan(&environment).unwrap();
        assert_eq!(plan.folder, std::path::PathBuf::from("/a"));
        assert_eq!(plan.extra, vec![std::path::PathBuf::from("/b"), "/c".into()]);
        assert_eq!(plan.env, EnvVars(vec![("X".to_string(), "1".to_string()), ("Y".to_string(), "two words".to_string())]));
        assert_eq!(plan.worktree, Some(NewWorktree { base: Some("dev".to_string()) }));
    }

    #[test]
    fn the_start_plan_refuses_an_environment_with_no_folder_or_with_bad_variables() {
        let mut environment = env("a", "app", false, 0);
        environment.folders.clear();
        assert_eq!(start_plan(&environment).unwrap_err(), StartError::NoFolder);
        let mut bad = env("b", "app", false, 0);
        bad.env_vars = "oops".to_string();
        assert_eq!(start_plan(&bad).unwrap_err(), StartError::Variables(EnvParseError { line: 1, reason: EnvParseReason::NotAPair }));
        let mut plain = env("c", "app", false, 0);
        plain.worktree = false;
        assert_eq!(start_plan(&plain).unwrap().worktree, None);
    }
}
```

In `crates/forge-features/src/shortcut.rs`, in its `#[cfg(test)]` module add:

```rust
    #[test]
    fn option_command_digits_pick_an_environment_and_option_command_zero_makes_one() {
        let press = |code: &'static str| KeyPress { code, ctrl: false, meta: true, alt: true, shift: false };
        assert_eq!(shortcut_for(&press("Digit1"), true), Some(Shortcut::PickEnvironment(0)));
        assert_eq!(shortcut_for(&press("Digit9"), true), Some(Shortcut::PickEnvironment(8)));
        assert_eq!(shortcut_for(&press("Digit0"), true), Some(Shortcut::NewEnvironment));
        let plain = KeyPress { code: "Digit2", ctrl: false, meta: true, alt: false, shift: false };
        assert_eq!(shortcut_for(&plain, true), Some(Shortcut::GoToSession(1)));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p forge-features environment option_command 2>&1 | tail -15`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

Above the tests in `crates/forge-features/src/environment.rs`:

```rust
//! What the environment menu, the environment dialog and a send from an environment decide, apart
//! from any signal, so each rule is tested on its own.

use std::path::PathBuf;

use forge_domain::{new_environment_id, parse_env, EnvParseError, EnvVars, Environment};

use crate::worktree_view::NewWorktree;

/// One row of the environment menu. `id` is `None` for this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvRow {
    pub id: Option<String>,
    pub name: String,
    /// The key that picks it (⌥⌘`number`), when it has one.
    pub number: Option<usize>,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentMenu {
    pub visible: Vec<EnvRow>,
    pub more: Vec<EnvRow>,
}

/// ⌥⌘1 is this computer, so at most eight saved environments are numbered.
const MAX_SAVED_NUMBERED: usize = 8;

// Pinned first, then the one used last; a name breaks a tie.
fn ordered(environments: &[Environment]) -> Vec<&Environment> {
    let mut list: Vec<&Environment> = environments.iter().collect();
    list.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.last_used_at.cmp(&a.last_used_at)).then_with(|| a.name.cmp(&b.name)));
    list
}

// The saved environments shown before `More`: every pinned one and the newest other one, at most
// eight; the rest stay under `More`.
fn split(environments: &[Environment]) -> (Vec<&Environment>, Vec<&Environment>) {
    let mut visible: Vec<&Environment> = Vec::new();
    let mut more: Vec<&Environment> = Vec::new();
    let mut unpinned_shown = false;
    for environment in ordered(environments) {
        let shown = if environment.pinned {
            true
        } else if !unpinned_shown {
            unpinned_shown = true;
            true
        } else {
            false
        };
        if shown && visible.len() < MAX_SAVED_NUMBERED {
            visible.push(environment);
        } else {
            more.push(environment);
        }
    }
    (visible, more)
}

fn row(environment: &Environment, number: Option<usize>) -> EnvRow {
    EnvRow { id: Some(environment.id.clone()), name: environment.name.clone(), number, pinned: environment.pinned }
}

/// The menu for `query`: with an empty query this computer, the visible saved environments (numbered
/// 1, 2, …) and the rest under `more`; with a query every match, flat and unnumbered, newest first.
pub fn environment_menu(environments: &[Environment], query: &str, this_computer: &str) -> EnvironmentMenu {
    let query = query.trim().to_lowercase();
    if !query.is_empty() {
        let matches = ordered(environments).into_iter().filter(|environment| environment.name.to_lowercase().contains(&query));
        let mut visible: Vec<&Environment> = matches.collect();
        visible.sort_by(|a, b| b.last_used_at.cmp(&a.last_used_at).then_with(|| a.name.cmp(&b.name)));
        return EnvironmentMenu { visible: visible.into_iter().map(|environment| row(environment, None)).collect(), more: Vec::new() };
    }
    let (shown, more) = split(environments);
    let mut visible = vec![EnvRow { id: None, name: this_computer.to_string(), number: Some(1), pinned: false }];
    visible.extend(shown.into_iter().enumerate().map(|(index, environment)| row(environment, Some(index + 2))));
    EnvironmentMenu { visible, more: more.into_iter().map(|environment| row(environment, None)).collect() }
}

/// The ids the keys ⌥⌘1…⌥⌘9 pick, in order; index 0 is this computer (`None`).
pub fn numbered_ids(environments: &[Environment]) -> Vec<Option<String>> {
    let (shown, _) = split(environments);
    std::iter::once(None).chain(shown.into_iter().map(|environment| Some(environment.id.clone()))).collect()
}

/// What the dialog edits. `id` is `None` while a new environment is being made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentDraft {
    pub id: Option<String>,
    pub name: String,
    pub folders: Vec<PathBuf>,
    pub branch: Option<String>,
    pub worktree: bool,
    pub env_vars: String,
}

impl EnvironmentDraft {
    /// A new environment: the name and folder suggested, a worktree on (as in the folder menu).
    pub fn blank(name: String, folder: Option<PathBuf>) -> Self {
        Self { id: None, name, folders: folder.into_iter().collect(), branch: None, worktree: true, env_vars: String::new() }
    }

    pub fn of(environment: &Environment) -> Self {
        Self {
            id: Some(environment.id.clone()),
            name: environment.name.clone(),
            folders: environment.folders.clone(),
            branch: environment.branch.clone(),
            worktree: environment.worktree,
            env_vars: environment.env_vars.clone(),
        }
    }

    pub fn changed_from(&self, initial: &EnvironmentDraft) -> bool {
        self != initial
    }

    pub fn variables_error(&self) -> Option<EnvParseError> {
        parse_env(&self.env_vars).err()
    }

    pub fn can_save(&self) -> bool {
        !self.name.trim().is_empty() && self.variables_error().is_none()
    }

    /// The environment to store. An edit keeps the id, the pin and the times of `previous`; a new one
    /// gets an id from `id_seed` and `now` as both times.
    pub fn build(&self, previous: Option<&Environment>, id_seed: u64, now: i64) -> Environment {
        Environment {
            id: previous.map_or_else(|| new_environment_id(id_seed), |environment| environment.id.clone()),
            name: self.name.clone(),
            folders: self.folders.clone(),
            branch: self.branch.clone(),
            worktree: self.worktree,
            env_vars: self.env_vars.clone(),
            pinned: previous.is_some_and(|environment| environment.pinned),
            created_at: previous.map_or(now, |environment| environment.created_at),
            last_used_at: previous.map_or(now, |environment| environment.last_used_at),
        }
        .normalized()
    }
}

/// What a send from an environment starts with.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentStart {
    pub folder: PathBuf,
    /// Folders besides the first, none equal to it, each once.
    pub extra: Vec<PathBuf>,
    pub env: EnvVars,
    pub worktree: Option<NewWorktree>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartError {
    NoFolder,
    Variables(EnvParseError),
}

pub fn start_plan(environment: &Environment) -> Result<EnvironmentStart, StartError> {
    let normalized = environment.clone().normalized();
    let mut folders = normalized.folders.into_iter();
    let folder = folders.next().ok_or(StartError::NoFolder)?;
    let env = parse_env(&normalized.env_vars).map_err(StartError::Variables)?;
    Ok(EnvironmentStart {
        folder,
        extra: folders.collect(),
        env: EnvVars(env),
        worktree: normalized.worktree.then(|| NewWorktree { base: normalized.branch }),
    })
}

/// A key the new-session screen's environment chip answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvironmentKey {
    /// ⌥⌘1…⌥⌘9, counted from 0 (0 is this computer).
    Pick(usize),
    /// ⌥⌘0.
    New,
}

/// What opens the environment dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentRequest {
    New { folder: Option<PathBuf> },
    Edit(String),
}
```

`crates/forge-features/src/lib.rs`: add `mod environment;` among the `mod` lines and, with the `pub use` lines:

```rust
pub use environment::{environment_menu, numbered_ids, start_plan, EnvRow, EnvironmentDraft, EnvironmentKey, EnvironmentMenu, EnvironmentRequest, EnvironmentStart, StartError};
```

and add `PickEnvironment`/`NewEnvironment` to nothing else there (`Shortcut` is already exported).

`crates/forge-features/src/shortcut.rs`:
1. In `enum Shortcut` after `OpenPicker(PickerKind),`:

```rust
    /// ⌥⌘1…9 on the new-session screen: picks the environment in that row of the menu (0 is this computer).
    PickEnvironment(usize),
    /// ⌥⌘0: opens the new-environment dialog.
    NewEnvironment,
```

2. In `SHORTCUTS`, after the `Open the effort menu` row:

```rust
    ShortcutRow { group: Group::Composer, label: msg!("Pick an environment 1…9"), action: Shortcut::PickEnvironment(0), keys: &[cmd_alt(DIGITS)] },
    ShortcutRow { group: Group::Composer, label: msg!("New environment"), action: Shortcut::NewEnvironment, keys: &[cmd_alt("Digit0")] },
```

3. In `key_label`, add `"Digit0" => "0".to_string(),`.
4. In `shortcut_for`, replace the body of the `if binding.code == DIGITS { … }` branch:

```rust
            if binding.code == DIGITS {
                if let Some(index) = digit_index(press.code) {
                    return Some(match row.action {
                        Shortcut::PickEnvironment(_) => Shortcut::PickEnvironment(index),
                        _ => Shortcut::GoToSession(index),
                    });
                }
            } else if binding.code == press.code {
```

`crates/forge-features/src/workspace.rs`:
1. Fields (next to `worktrees: Signal<Vec<WorktreeRecord>>,` at ~154) and their `Signal::new` initialisers (next to `worktrees: Signal::new(Vec::new()),` at ~224):

```rust
    environments: Signal<Vec<forge_domain::Environment>>,
    environment_key: Signal<Option<crate::EnvironmentKey>>,
    environment_request: Signal<Option<crate::EnvironmentRequest>>,
```
```rust
            environments: Signal::new(Vec::new()),
            environment_key: Signal::new(None),
            environment_request: Signal::new(None),
```

2. In `reload_prefs`: `let mut environments = self.environments;` with the other `let mut` lines and `environments.set(config.environments);` after `worktrees.set(config.worktrees);`.

3. Methods (put them together after `clear_picker_request`):

```rust
    pub fn environments(&self) -> Vec<forge_domain::Environment> {
        self.environments.read().clone()
    }

    /// Adds the environment, or replaces the one with its id. Pinned to the root scope: the save must
    /// finish even if the dialog goes away meanwhile.
    pub fn save_environment(&self, environment: forge_domain::Environment) {
        self.change_environments(move |list| match list.iter_mut().find(|existing| existing.id == environment.id) {
            Some(existing) => *existing = environment,
            None => list.push(environment),
        });
    }

    pub fn delete_environment(&self, id: String) {
        self.change_environments(move |list| list.retain(|environment| environment.id != id));
    }

    pub fn set_environment_pinned(&self, id: String, pinned: bool) {
        self.change_environments(move |list| {
            if let Some(environment) = list.iter_mut().find(|environment| environment.id == id) {
                environment.pinned = pinned;
            }
        });
    }

    fn change_environments(&self, change: impl FnOnce(&mut Vec<forge_domain::Environment>) + 'static) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        dioxus::core::spawn_forever(async move {
            match services.settings.save_code_settings(|config| change(&mut config.environments)).await {
                Ok(config) => {
                    let mut environments = this.environments;
                    environments.set(config.environments);
                }
                Err(error) => this.report(tr!("Could not save the environment: {error}", error = error)),
            }
        });
    }

    /// Asks the new-session screen's environment chip to act on a key (⌥⌘0…9). Nothing else shows one.
    pub fn request_environment_key(&self, key: crate::EnvironmentKey) {
        if self.active() == Active::NewSession {
            let mut request = self.environment_key;
            request.set(Some(key));
        }
    }

    pub fn environment_key(&self) -> Option<crate::EnvironmentKey> {
        *self.environment_key.read()
    }

    pub fn clear_environment_key(&self) {
        let mut request = self.environment_key;
        if request.peek().is_some() {
            request.set(None);
        }
    }

    pub fn environment_request(&self) -> Option<crate::EnvironmentRequest> {
        self.environment_request.read().clone()
    }

    pub fn open_environment_dialog(&self, request: crate::EnvironmentRequest) {
        let mut current = self.environment_request;
        current.set(Some(request));
    }

    pub fn close_environment_dialog(&self) {
        let mut current = self.environment_request;
        current.set(None);
    }

    /// The local branches of `folder`; `None` when it is not a git repository.
    pub async fn folder_branches(&self, folder: PathBuf) -> Option<forge_ports::BranchList> {
        let services = (*self.services.peek()).clone();
        services.worktrees.branches(folder).await.ok()
    }

    /// The system folder dialog; `None` when cancelled.
    pub async fn pick_folder(&self) -> Option<PathBuf> {
        let services = (*self.services.peek()).clone();
        services.folder_picker.pick_folder().await
    }

    /// Whether `folder` is off the trusted list, so the person must be asked. A list that cannot be
    /// read counts as empty, like the composer's add-folder.
    pub async fn needs_trust(&self, folder: PathBuf) -> bool {
        let services = (*self.services.peek()).clone();
        let trusted = services.settings.load().await.map(|config| config.trusted_folders).unwrap_or_default();
        crate::run_blocking(move || crate::trust::classify_added_folder(&trusted, &[], &folder, &crate::trust::resolved(&folder)) == crate::trust::FolderAdd::NeedsAsking).await.unwrap_or(true)
    }

    /// Puts `folder` on the trusted list.
    pub fn trust_folder(&self, folder: PathBuf) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        dioxus::core::spawn_forever(async move {
            let Some(resolved) = crate::run_blocking(move || crate::trust::resolved(&folder)).await else { return };
            let saved = services.settings.save_code_settings(|config| forge_domain::add_trusted(&mut config.trusted_folders, resolved));
            if let Err(error) = saved.await {
                this.report(tr!("Could not save the trusted folder: {error}", error = error));
            }
        });
    }
```

(`trust` is the module `crates/forge-features/src/trust.rs`; `run_blocking` is exported at the crate root.)

4. In the shortcut `match` of the key handler, after `Shortcut::OpenPicker(kind) => self.request_picker(kind),`:

```rust
            Shortcut::PickEnvironment(index) => self.request_environment_key(crate::EnvironmentKey::Pick(index)),
            Shortcut::NewEnvironment => self.request_environment_key(crate::EnvironmentKey::New),
```

5. `start_new` gets the environment. Change its signature's tail and body: add the parameter `environment: Option<crate::EnvironmentStart>,` after `worktree: Option<NewWorktree>,`. At the top of the `spawn_forever` block, before `let project = match worktree {`, add:

```rust
            // An environment names its worktree choice, not a probed repository: ask git now, and start
            // in the folder itself when it is not a repository.
            let worktree = match (&environment, worktree) {
                (Some(_), Some(request)) => services.worktrees.branches(project.path.clone()).await.ok().map(|_| request),
                (_, other) => other,
            };
```

and after `let id = this.attach(handle.clone());`, replace the `if !folders.is_empty() { handle.add_directories(folders); }` block by:

```rust
                    let mut folders = folders;
                    if let Some(environment) = &environment {
                        for extra in &environment.extra {
                            if !folders.contains(extra) {
                                folders.push(extra.clone());
                            }
                        }
                        if !environment.env.0.is_empty() {
                            handle.set_env(environment.env.clone());
                        }
                    }
                    if !folders.is_empty() {
                        handle.add_directories(folders);
                    }
```

6. Touch `last_used_at`: in the same `Ok(handle)` arm, add (the environment's id is needed, so extend `EnvironmentStart` is not necessary: pass it through `new_session.rs`, see below) nothing here. `new_session.rs` calls `workspace.touch_environment(id)` right after `start_new` — add this method next to `set_environment_pinned`:

```rust
    /// Marks the environment as used now, which is what orders the menu.
    pub fn touch_environment(&self, id: String) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_millis() as i64).unwrap_or(0);
        self.change_environments(move |list| {
            if let Some(environment) = list.iter_mut().find(|environment| environment.id == id) {
                environment.last_used_at = now;
            }
        });
    }
```

`crates/forge-features/src/new_session.rs`:
1. `NewSessionDraft` gets `pub environment: Signal<Option<String>>,`; its construction in `NewSessionComposer` gets `environment: Signal::new(None),`.
2. `has_folder` becomes:

```rust
    let picked_environment = draft.environment.read().clone().and_then(|id| workspace.environments().into_iter().find(|environment| environment.id == id));
    let has_folder = picked_environment.is_some() || !workspace.new_session_projects().is_empty();
```

3. In `submit`, replace the block from `let projects = workspace.new_session_projects();` through the `let Some(project) = project else { return; };` and the existing `workspace.start_new(...)` call by:

```rust
        let environment = draft.environment.peek().clone().and_then(|id| workspace.environments().into_iter().find(|environment| environment.id == id));
        let (project, plan) = match &environment {
            Some(environment) => match start_plan(environment) {
                Ok(plan) => {
                    let exists = plan.folder.is_dir();
                    (ProjectSummary { path: plan.folder.clone(), last_active: String::new(), session_count: 0, exists }, Some(plan))
                }
                Err(StartError::NoFolder) => {
                    workspace.report(tr!("Add a folder to this environment first").to_string());
                    return;
                }
                Err(StartError::Variables(error)) => {
                    workspace.report(tr!("The variables of this environment are not valid (line {line}). Edit the environment.", line = error.line));
                    return;
                }
            },
            None => {
                let projects = workspace.new_session_projects();
                let project = match workspace.new_folder() {
                    Some(path) => projects.iter().find(|project| project.path == path).cloned().or_else(|| projects.first().cloned()),
                    None => projects.first().cloned(),
                };
                let Some(project) = project else {
                    return;
                };
                (project, None)
            }
        };
        let Some(outgoing) = composer.take_for_new_session(&text.read()) else {
            return;
        };
        let worktree = match &plan {
            Some(plan) => plan.worktree.clone(),
            None => worktree_request(*draft.worktree.peek(), draft.branch.peek().clone(), workspace.new_repo(), &project.path),
        };
        workspace.start_new(
            project,
            *draft.mode.peek(),
            draft.model.peek().clone(),
            *draft.effort.peek(),
            outgoing.text,
            outgoing.attachments,
            composer.take_folders(),
            worktree,
            plan,
        );
        if let Some(environment) = environment {
            workspace.touch_environment(environment.id);
        }
```

(add `use forge_domain::ProjectSummary;` and `use crate::environment::{start_plan, StartError};` to the imports; keep the existing `take_for_new_session` position: it must still run before `start_new` and after the checks, as above.)

4. Also drop an environment that no longer exists: after the `use_new_session_draft`-style effects add to `NewSessionComposer`:

```rust
    // An environment deleted while it was picked: back to this computer.
    use_effect(move || {
        let picked = draft.environment.read().clone();
        if let Some(id) = picked {
            if !workspace.environments().iter().any(|environment| environment.id == id) {
                let mut picked_signal = draft.environment;
                picked_signal.set(None);
            }
        }
    });
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p forge-features 2>&1 | tail -12`
Expected: PASS (the new environment and shortcut tests, and every existing one — the shortcut table tests may check that no two rows share a key or that every row is listed; the two new rows use `Alt+Cmd+digit`, which nothing else uses).
Run: `cargo build --workspace 2>&1 | tail -5`
Expected: builds (the widget crates do not use the new API yet; `start_new`'s only caller is `new_session.rs`).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features
git commit -m "feat: environment menu order, dialog draft, start plan and keys

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: The environment chip and menu

**Files:**
- Create: `crates/forge-widgets/src/environment_chip.rs`
- Modify: `crates/forge-widgets/src/lib.rs` (`mod environment_chip;`)
- Modify: `crates/forge-widgets/src/new_session_panel.rs`, `new_session_controls.rs`
- Modify: `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: everything from Task 3 (`environment_menu`, `numbered_ids`, `EnvironmentKey`, `EnvironmentRequest`, `NewSessionDraft.environment`, workspace methods).
- Produces: `EnvironmentChip` component (no props). While an environment is picked, `FolderChip`, `BranchChip` and `AddFolderButton` render nothing.

- [ ] **Step 1: Write the failing test**

The chip is drawing code over tested logic. The one rule worth a unit test here is "picking sets the draft's worktree and branch from the environment"; put it as a pure function in `crates/forge-features/src/environment.rs` (Task 3's file) and test it there. Add to its tests module:

```rust
    #[test]
    fn picking_an_environment_takes_its_worktree_choice_and_this_computer_leaves_the_draft_as_it_is() {
        let mut environment = env("a", "app", false, 0);
        environment.worktree = true;
        environment.branch = Some("dev".to_string());
        assert_eq!(draft_choice(Some(&environment)), Some((true, Some("dev".to_string()))));
        assert_eq!(draft_choice(None), None);
    }
```

Run: `cargo test -p forge-features picking_an_environment 2>&1 | tail -6` — Expected: FAIL (`draft_choice` not found).

Add to `crates/forge-features/src/environment.rs` (above the tests) and export it in `lib.rs`'s `environment::{…}` list:

```rust
/// The worktree switch and branch a picked environment puts in the draft; `None` for this computer,
/// which leaves the draft alone.
pub fn draft_choice(environment: Option<&Environment>) -> Option<(bool, Option<String>)> {
    environment.map(|environment| (environment.worktree, environment.branch.clone()))
}
```

Run: `cargo test -p forge-features picking_an_environment 2>&1 | tail -6` — Expected: PASS.

- [ ] **Step 2: Write the chip**

`crates/forge-widgets/src/environment_chip.rs`:

```rust
use dioxus::prelude::*;
use forge_domain::Environment;
use forge_features::{draft_choice, environment_menu, numbered_ids, refocus_composer, use_new_session_draft, use_workspace, EnvRow, EnvironmentKey, EnvironmentRequest};
use forge_shared::tr;

/// The chip at the left of the new-session chips: the environment the session starts from, and a menu
/// of this computer and the saved environments. ⌥⌘1…9 pick a row, ⌥⌘0 makes a new one.
#[component]
pub fn EnvironmentChip() -> Element {
    let workspace = use_workspace();
    let mut draft = use_new_session_draft();
    let mut open = use_signal(|| false);
    let mut more = use_signal(|| false);
    let mut query = use_signal(String::new);
    let environments = workspace.environments();
    let this_computer = tr!("This computer").to_string();
    let picked: Option<Environment> = draft.environment.read().clone().and_then(|id| environments.iter().find(|environment| environment.id == id).cloned());
    let label = picked.as_ref().map_or_else(|| this_computer.clone(), |environment| environment.name.clone());

    let mut pick = move |id: Option<String>| {
        let list = workspace.environments();
        let chosen = id.as_ref().and_then(|id| list.iter().find(|environment| environment.id == *id));
        if let Some((worktree, branch)) = draft_choice(chosen) {
            draft.worktree.set(worktree);
            draft.branch.set(branch);
        }
        draft.environment.set(id);
        open.set(false);
        more.set(false);
        query.set(String::new());
        refocus_composer();
    };

    // The keys ⌥⌘0…9 land here.
    use_effect(move || {
        let Some(key) = workspace.environment_key() else { return };
        workspace.clear_environment_key();
        match key {
            EnvironmentKey::Pick(index) => {
                if let Some(id) = numbered_ids(&workspace.environments()).get(index).cloned() {
                    pick(id);
                }
            }
            EnvironmentKey::New => workspace.open_environment_dialog(EnvironmentRequest::New { folder: workspace.chosen_new_folder() }),
        }
    });

    let menu = environment_menu(&environments, &query.read(), &this_computer);
    let searching = !query.read().trim().is_empty();
    rsx! {
        div { class: "picker env-chip",
            button {
                class: if open() { "picker-button open" } else { "picker-button" },
                r#type: "button",
                title: tr!("Environment"),
                "aria-haspopup": "menu",
                "aria-expanded": "{open()}",
                onclick: move |_| open.set(!open()),
                span { "{label}" }
                span { class: "picker-chevron", "▾" }
            }
            if open() {
                div {
                    class: "picker-backdrop",
                    onclick: move |_| {
                        open.set(false);
                        more.set(false);
                        refocus_composer();
                    },
                }
                div {
                    class: "picker-menu env-menu",
                    role: "menu",
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.stop_propagation();
                            open.set(false);
                            more.set(false);
                            refocus_composer();
                        }
                    },
                    input {
                        class: "env-search",
                        r#type: "text",
                        placeholder: tr!("Search environments…"),
                        value: "{query}",
                        onmounted: move |mounted| async move {
                            let _ = mounted.set_focus(true).await;
                        },
                        oninput: move |event| query.set(event.value()),
                    }
                    if !searching {
                        div { class: "picker-title", {tr!("Local")} }
                    }
                    for row in menu.visible.iter().cloned() {
                        EnvironmentRow { key: "{row.id:?}", row: row.clone(), checked: row.id == picked.as_ref().map(|environment| environment.id.clone()), onpick: move |id: Option<String>| pick(id) }
                    }
                    if !menu.more.is_empty() {
                        button {
                            class: "picker-item env-more",
                            r#type: "button",
                            onclick: move |_| more.set(!more()),
                            span { class: "picker-check" }
                            span { class: "picker-text", span { class: "picker-label", {tr!("More")} } }
                            span { class: "menu-key", "{menu.more.len()} ›" }
                        }
                        if more() {
                            for row in menu.more.iter().cloned() {
                                EnvironmentRow { key: "{row.id:?}", row: row.clone(), checked: row.id == picked.as_ref().map(|environment| environment.id.clone()), onpick: move |id: Option<String>| pick(id) }
                            }
                        }
                    }
                    div { class: "picker-divider" }
                    button {
                        class: "picker-item",
                        r#type: "button",
                        onclick: move |_| {
                            open.set(false);
                            workspace.open_environment_dialog(EnvironmentRequest::New { folder: workspace.chosen_new_folder() });
                        },
                        span { class: "picker-check", "+" }
                        span { class: "picker-text", span { class: "picker-label", {tr!("New environment")} } }
                        span { class: "menu-key", {new_key_label()} }
                    }
                }
            }
        }
    }
}

fn new_key_label() -> &'static str {
    if cfg!(target_os = "macos") { "⌥⌘0" } else { "Ctrl+Alt+0" }
}

fn number_label(number: usize) -> String {
    if cfg!(target_os = "macos") { format!("⌥⌘{number}") } else { format!("Ctrl+Alt+{number}") }
}

#[component]
fn EnvironmentRow(row: EnvRow, checked: bool, onpick: EventHandler<Option<String>>) -> Element {
    let workspace = use_workspace();
    let id = row.id.clone();
    let edit_id = row.id.clone();
    let star_id = row.id.clone();
    let pinned = row.pinned;
    rsx! {
        div { class: if checked { "env-row checked" } else { "env-row" },
            button {
                class: if checked { "picker-item checked" } else { "picker-item" },
                r#type: "button",
                role: "menuitemradio",
                "aria-checked": "{checked}",
                onclick: move |_| onpick.call(id.clone()),
                span { class: "picker-check", if checked { "✓" } }
                span { class: "picker-text", span { class: "picker-label", "{row.name}" } }
                if let Some(number) = row.number {
                    span { class: "menu-key", {number_label(number)} }
                }
            }
            if let Some(edit_id) = edit_id {
                span { class: "env-row-icons",
                    button {
                        class: "env-icon",
                        r#type: "button",
                        title: tr!("Edit environment"),
                        onclick: move |_| workspace.open_environment_dialog(EnvironmentRequest::Edit(edit_id.clone())),
                        "⚙"
                    }
                    button {
                        class: if pinned { "env-icon pinned" } else { "env-icon" },
                        r#type: "button",
                        title: if pinned { tr!("Unpin") } else { tr!("Pin") },
                        onclick: move |_| {
                            if let Some(star_id) = star_id.clone() {
                                workspace.set_environment_pinned(star_id, !pinned);
                            }
                        },
                        if pinned { "★" } else { "☆" }
                    }
                }
            }
        }
    }
}
```

Notes for the implementer: `tr!` returns `&'static str`; `title: tr!(…)` is accepted as in `new_session_controls.rs`. `Picker`'s own CSS classes are reused; only the classes below are new.

`crates/forge-widgets/src/lib.rs`: add `mod environment_chip;` beside `mod new_session_controls;`.

`crates/forge-widgets/src/new_session_panel.rs`: import `use crate::environment_chip::EnvironmentChip;` and make the chips

```rust
                chips: rsx! {
                    EnvironmentChip {}
                    FolderChip {}
                    BranchChip {}
                    AddFolderButton {}
                },
```

`crates/forge-widgets/src/new_session_controls.rs`: at the top of `FolderChip`, `BranchChip` and `AddFolderButton` (all of them render under `NewSessionComposer`'s draft context; `FolderChip` and `BranchChip` already call `use_new_session_draft()` — in `AddFolderButton` add the call), return early while an environment is picked:

```rust
    if draft.environment.read().is_some() {
        return rsx! {};
    }
```

For `FolderChip` this line goes after all hooks (after `let mut draft = use_new_session_draft();` and before building `items`, the hook calls `use_workspace()`/`use_new_session_draft()` must come first so the hook order does not change).

`crates/forge-shared/src/theme.css`: append (use the variables the neighbouring `.picker-*` rules use; look at them and copy their colour variables):

```css
.env-menu { min-width: 260px; }
.env-search { width: calc(100% - 16px); margin: 6px 8px; padding: 6px 8px; border: 1px solid var(--border); border-radius: 8px; background: transparent; color: inherit; font: inherit; }
.env-row { position: relative; display: flex; align-items: center; }
.env-row > .picker-item { flex: 1; }
.env-row-icons { position: absolute; right: 56px; display: none; gap: 2px; }
.env-row:hover .env-row-icons { display: inline-flex; }
.env-icon { border: 0; background: transparent; color: var(--muted); cursor: pointer; padding: 2px 4px; border-radius: 4px; }
.env-icon:hover { background: var(--hover); color: inherit; }
.env-icon.pinned { color: var(--accent); }
```

(If the stylesheet has no `--border`, `--muted`, `--hover`, `--accent`, use the variables of `.picker-menu` and `.picker-item:hover`.)

- [ ] **Step 3: Check it compiles and runs**

Run: `cargo build --workspace 2>&1 | tail -15` — Expected: builds. Fix type errors in the widget by following the compiler (the likely ones: `key: "{row.id:?}"` may need `key: "{row.name}"`; the `pick` closure is `Copy`-captured in several places — use `let pick = move |…|` and clone the signals it uses, as `Picker` does).
Run: `cargo test -p forge-features -p forge-widgets 2>&1 | tail -6` — Expected: PASS.

- [ ] **Step 4: Real-window check of the menu (harness)**

Run `scripts/harness/launch.sh`, then `python3 scripts/harness/capture.py` and read `/private/tmp/forge-harness-shot.png`. Verify with `python3 scripts/harness/click.py X Y` (pixels of the PNG times the viewer's scale factor):
- the new-session screen shows the chip `This computer` left of the folder chip;
- opening it shows the search field, the group title `Local`, `This computer ✓ ⌥⌘1`, `+ New environment ⌥⌘0`; no cloud or Remote Control row;
- pressing ⌥⌘0 opens nothing yet if Task 5 is not built (the dialog request is set; that is fine);
- Escape and a click outside close the menu.
Stop with `scripts/harness/stop.sh`. (Saved environments are checked in Task 5.)

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat: environment chip and menu on the new-session screen

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The environment dialog

**Files:**
- Create: `crates/forge-widgets/src/environment_dialog.rs`
- Modify: `crates/forge-widgets/src/lib.rs` (`mod environment_dialog; pub use environment_dialog::EnvironmentDialog;`)
- Modify: `crates/forge-pages/src/workspace_page.rs` (import and mount next to `ShortcutsDialog {}`)
- Modify: `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: `EnvironmentDraft`, `EnvironmentRequest`, `Workspace::{environment_request, close_environment_dialog, save_environment, delete_environment, folder_branches, pick_folder, needs_trust, trust_folder, new_session_projects}` (Task 3).
- Produces: `EnvironmentDialog` component (no props); renders nothing while `environment_request()` is `None`.

Behaviour (from the spec's observations):
- Title `New environment` / `Edit environment`; field `Name`; `Folders (optional)` with `+ Add`; each folder row shows the name and, on hover, `×`. The first folder's row, when it is a git repository, shows its branch picker (a `<select>`; list from `folder_branches`) and a `Worktree` checkbox; a first folder that is not a repository shows `Not a Git repository`. Later folders are plain rows. No folder shows the dashed text `Choose a folder to start the session in.`.
- `+ Add` opens a popover: a search field, the known folders (`new_session_projects` paths that exist and are not in the list), and `Choose…` (system dialog through `pick_folder`). A folder that `needs_trust` shows the trust question inside the dialog (`Cancel` / `Trust and add`) before it is added.
- A card `Variables` with the sub-line `None in this environment` (or `N variables`) and `›` opens the variables page: back arrow, heading, a mono text area with the `.env` placeholder, the note, `Done`. Parse errors show under the text area as `Line {line}: {reason}` and `Done` stays disabled.
- Buttons: `Cancel`, `Create environment` (new) / `Save` (edit), disabled while the draft cannot be saved. Edit adds `Delete environment` (red text, left). Closing a changed dialog (✕, Escape, backdrop, `Cancel`) asks `Create this environment?` / `Save the changes?` with `Discard`, `Keep editing` and the save button; closing an unchanged one just closes it.
- `Delete environment` asks `Delete environment?` — “{name}” will be deleted for good. Sessions started from it are kept. — with `Cancel` and `Delete environment`. Deleting the picked environment needs no extra code (the composer's effect in Task 3 falls back to this computer).
- The name of a new environment is the first folder's name (the folder in `EnvironmentRequest::New`), or empty; it is selected when the dialog opens.
- The note on the variables page: `Saved in Forge's settings file on this computer. Written in .env format and passed to Claude sessions started from this environment.`

- [ ] **Step 1: Write the failing test (reason texts)**

The reason→text mapping is a pure function; add it to `crates/forge-features/src/environment.rs` so it is tested, then used by the widget:

```rust
/// The one-line text for a bad variables line, for the dialog.
pub fn variables_error_text(error: &EnvParseError) -> String {
    use forge_domain::EnvParseReason::{BadKey, NotAPair, UnclosedQuote};
    match error.reason {
        NotAPair => forge_shared::tr!("Line {line}: expected NAME=value", line = error.line),
        BadKey => forge_shared::tr!("Line {line}: the name may use letters, digits and _ and must not start with a digit", line = error.line),
        UnclosedQuote => forge_shared::tr!("Line {line}: a quote is not closed", line = error.line),
    }
}
```

and its test:

```rust
    #[test]
    fn each_reason_has_its_own_text_with_the_line_number() {
        use forge_domain::EnvParseReason::{BadKey, NotAPair, UnclosedQuote};
        let texts: Vec<String> = [NotAPair, BadKey, UnclosedQuote].into_iter().map(|reason| variables_error_text(&EnvParseError { line: 7, reason })).collect();
        assert!(texts.iter().all(|text| text.contains('7')));
        assert_eq!(texts.iter().collect::<std::collections::HashSet<_>>().len(), 3);
    }
```

Run: `cargo test -p forge-features each_reason 2>&1 | tail -6` — Expected: FAIL; add the function and export `variables_error_text` from `lib.rs`; Expected after: PASS. (`tr!` with named arguments is used the same way in `workspace.rs`: `tr!("… {error}", error = error)`.)

- [ ] **Step 2: Write the dialog**

`crates/forge-widgets/src/environment_dialog.rs`:

```rust
use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use forge_features::{refocus_workspace, use_workspace, variables_error_text, EnvironmentDraft, EnvironmentRequest};
use forge_shared::{tr, Button, ButtonKind};

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Form,
    Variables,
}

#[derive(Clone, Copy, PartialEq)]
enum Ask {
    None,
    /// Closing a changed dialog.
    Discard,
    Delete,
}

fn folder_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().to_string())
}

/// `새 환경` and `환경 편집`: one dialog for both. Renders nothing while no request is open.
#[component]
pub fn EnvironmentDialog() -> Element {
    let workspace = use_workspace();
    let Some(request) = workspace.environment_request() else {
        return rsx! {};
    };
    // A request is a different dialog: the key rebuilds the body with a fresh draft.
    let key = format!("{request:?}");
    rsx! { EnvironmentDialogBody { key: "{key}", request } }
}

#[component]
fn EnvironmentDialogBody(request: EnvironmentRequest) -> Element {
    let workspace = use_workspace();
    let stored = match &request {
        EnvironmentRequest::Edit(id) => workspace.environments().into_iter().find(|environment| environment.id == *id),
        EnvironmentRequest::New { .. } => None,
    };
    let initial = match (&request, &stored) {
        (EnvironmentRequest::Edit(_), Some(environment)) => EnvironmentDraft::of(environment),
        (EnvironmentRequest::New { folder }, _) => EnvironmentDraft::blank(folder.as_deref().map(folder_name).unwrap_or_default(), folder.clone()),
        // An environment deleted under an open edit dialog: nothing left to edit.
        (EnvironmentRequest::Edit(_), None) => {
            workspace.close_environment_dialog();
            return rsx! {};
        }
    };
    let editing = stored.is_some();
    let mut draft = use_signal({
        let initial = initial.clone();
        move || initial
    });
    let initial = use_signal(|| initial);
    let mut page = use_signal(|| Page::Form);
    let mut ask = use_signal(|| Ask::None);
    let mut adding = use_signal(|| false);
    let mut search = use_signal(String::new);
    let mut trust_question = use_signal(|| None::<PathBuf>);
    let mut variables_text = use_signal(String::new);

    // The branches of the first folder, when it is a git repository.
    let branches = use_resource(move || {
        let folder = draft.read().folders.first().cloned();
        async move {
            match folder {
                Some(folder) => workspace.folder_branches(folder).await,
                None => None,
            }
        }
    });

    let close = move || {
        workspace.close_environment_dialog();
        refocus_workspace();
    };
    let request_close = move || {
        if draft.read().changed_from(&initial.read()) {
            ask.set(Ask::Discard);
        } else {
            close();
        }
    };
    let save = {
        let stored = stored.clone();
        move || {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_millis() as i64).unwrap_or(0);
            let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_nanos() as u64).unwrap_or(0);
            let environment = draft.read().build(stored.as_ref(), seed, now);
            workspace.save_environment(environment);
            close();
        }
    };
    let can_save = draft.read().can_save();

    let add_folder = move |path: PathBuf| {
        spawn(async move {
            if draft.peek().folders.contains(&path) {
                adding.set(false);
                return;
            }
            if workspace.needs_trust(path.clone()).await {
                trust_question.set(Some(path));
            } else {
                draft.write().folders.push(path);
            }
            adding.set(false);
        });
    };

    rsx! {
        div {
            class: "settings-backdrop",
            onclick: move |_| request_close(),
            div {
                class: "env-dialog",
                role: "dialog",
                "aria-label": if editing { tr!("Edit environment") } else { tr!("New environment") },
                onclick: move |event| event.stop_propagation(),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape {
                        event.stop_propagation();
                        match page() {
                            Page::Variables => page.set(Page::Form),
                            Page::Form => request_close(),
                        }
                    }
                },
                if page() == Page::Variables {
                    div { class: "env-head",
                        button { class: "env-back", r#type: "button", title: tr!("Back"), onclick: move |_| page.set(Page::Form), "‹" }
                        h2 { {tr!("Variables")} }
                        button { class: "settings-close", r#type: "button", title: tr!("Close (Esc)"), onclick: move |_| request_close(), "✕" }
                    }
                    p { class: "hint", {tr!("Set for sessions started from this environment.")} }
                    textarea {
                        class: "env-variables",
                        spellcheck: "false",
                        placeholder: "NODE_ENV=production\nGIT_AUTHOR_NAME=Your Name\n# Multiline values - wrap in quotes\nCONFIG=\"key1=val1\nkey2=val2\"",
                        value: "{variables_text}",
                        oninput: move |event| variables_text.set(event.value()),
                    }
                    {
                        let error = EnvironmentDraft { env_vars: variables_text(), ..draft.read().clone() }.variables_error();
                        rsx! {
                            if let Some(error) = error.as_ref() {
                                div { class: "banner error", {variables_error_text(error)} }
                            }
                            p { class: "hint", {tr!("Saved in Forge's settings file on this computer. Written in .env format and passed to Claude sessions started from this environment.")} }
                            div { class: "env-buttons",
                                span { class: "spacer" }
                                Button {
                                    label: tr!("Done"),
                                    kind: ButtonKind::Primary,
                                    disabled: error.is_some(),
                                    onclick: move |_| {
                                        draft.write().env_vars = variables_text();
                                        page.set(Page::Form);
                                    },
                                }
                            }
                        }
                    }
                } else {
                    div { class: "env-head",
                        h2 { if editing { {tr!("Edit environment")} } else { {tr!("New environment")} } }
                        button { class: "settings-close", r#type: "button", title: tr!("Close (Esc)"), onclick: move |_| request_close(), "✕" }
                    }
                    label { class: "env-label", {tr!("Name")} }
                    input {
                        class: "customize-field",
                        r#type: "text",
                        value: "{draft.read().name}",
                        onmounted: move |mounted| async move {
                            let _ = mounted.set_focus(true).await;
                        },
                        oninput: move |event| draft.write().name = event.value(),
                    }
                    div { class: "env-folders-head",
                        label { class: "env-label", {tr!("Folders")} span { class: "hint", " " {tr!("(optional)")} } }
                        button { class: "env-add", r#type: "button", onclick: move |_| adding.set(!adding()), {tr!("+ Add")} }
                    }
                    if adding() {
                        FolderPopover { search, onpick: add_folder, onclose: move |_| adding.set(false), taken: draft.read().folders.clone() }
                    }
                    if let Some(path) = trust_question() {
                        div { class: "trust-card", tabindex: "0",
                            div { class: "trust-question", {tr!("Allow Claude to use this folder and its subfolders?")} }
                            div { class: "trust-path", title: "{path.display()}", "{path.display()}" }
                            div { class: "trust-buttons",
                                Button { label: tr!("Cancel"), onclick: move |_| trust_question.set(None) }
                                Button {
                                    label: tr!("Trust and add"),
                                    kind: ButtonKind::Primary,
                                    onclick: {
                                        let path = path.clone();
                                        move |_| {
                                            workspace.trust_folder(path.clone());
                                            draft.write().folders.push(path.clone());
                                            trust_question.set(None);
                                        }
                                    },
                                }
                            }
                        }
                    }
                    if draft.read().folders.is_empty() {
                        div { class: "env-empty", {tr!("Choose a folder to start the session in.")} }
                    } else {
                        div { class: "env-folders",
                            for (index, folder) in draft.read().folders.clone().into_iter().enumerate() {
                                div { key: "{folder.display()}", class: "env-folder", title: "{folder.display()}",
                                    span { class: "env-folder-name", "{folder_name(&folder)}" }
                                    if index == 0 {
                                        match branches.read().clone().flatten() {
                                            Some(list) => rsx! {
                                                select {
                                                    class: "env-branch",
                                                    value: "{draft.read().branch.clone().or_else(|| list.current.clone()).unwrap_or_default()}",
                                                    onchange: move |event| draft.write().branch = Some(event.value()),
                                                    for name in list.local.clone() {
                                                        option { key: "{name}", value: "{name}", "{name}" }
                                                    }
                                                }
                                                label { class: "env-worktree",
                                                    input {
                                                        r#type: "checkbox",
                                                        checked: draft.read().worktree,
                                                        onchange: move |event| draft.write().worktree = event.checked(),
                                                    }
                                                    {tr!("Worktree")}
                                                }
                                            },
                                            None => rsx! { span { class: "env-note", {tr!("Not a Git repository")} } },
                                        }
                                    }
                                    button {
                                        class: "env-remove",
                                        r#type: "button",
                                        title: tr!("Remove"),
                                        onclick: move |_| {
                                            let mut folders = draft.write();
                                            if index < folders.folders.len() {
                                                folders.folders.remove(index);
                                            }
                                        },
                                        "×"
                                    }
                                }
                            }
                        }
                    }
                    button {
                        class: "env-variables-card",
                        r#type: "button",
                        onclick: move |_| {
                            variables_text.set(draft.read().env_vars.clone());
                            page.set(Page::Variables);
                        },
                        span { class: "env-label", {tr!("Variables")} }
                        span { class: "hint", { variables_summary(&draft.read().env_vars) } }
                        span { class: "env-chevron", "›" }
                    }
                    div { class: "env-buttons",
                        if editing {
                            button { class: "env-delete", r#type: "button", onclick: move |_| ask.set(Ask::Delete), {tr!("Delete environment")} }
                        }
                        span { class: "spacer" }
                        Button { label: tr!("Cancel"), onclick: move |_| request_close() }
                        Button {
                            label: if editing { tr!("Save") } else { tr!("Create environment") },
                            kind: ButtonKind::Primary,
                            disabled: !can_save,
                            onclick: {
                                let mut save = save.clone();
                                move |_| save()
                            },
                        }
                    }
                }
                if ask() != Ask::None {
                    div { class: "env-ask-backdrop",
                        div { class: "env-ask", role: "alertdialog",
                            match ask() {
                                Ask::Delete => rsx! {
                                    h3 { {tr!("Delete environment?")} }
                                    p { {tr!("“{name}” will be deleted for good. Sessions started from it are kept.", name = stored.as_ref().map(|environment| environment.name.clone()).unwrap_or_default())} }
                                    div { class: "env-buttons",
                                        span { class: "spacer" }
                                        Button { label: tr!("Cancel"), onclick: move |_| ask.set(Ask::None) }
                                        Button {
                                            label: tr!("Delete environment"),
                                            kind: ButtonKind::Danger,
                                            onclick: {
                                                let id = stored.as_ref().map(|environment| environment.id.clone());
                                                move |_| {
                                                    if let Some(id) = id.clone() {
                                                        workspace.delete_environment(id);
                                                    }
                                                    close();
                                                }
                                            },
                                        }
                                    }
                                },
                                _ => rsx! {
                                    h3 { if editing { {tr!("Save the changes?")} } else { {tr!("Create this environment?")} } }
                                    div { class: "env-buttons",
                                        Button { label: tr!("Discard"), kind: ButtonKind::Danger, onclick: move |_| close() }
                                        span { class: "spacer" }
                                        Button { label: tr!("Keep editing"), onclick: move |_| ask.set(Ask::None) }
                                        Button {
                                            label: if editing { tr!("Save") } else { tr!("Create environment") },
                                            kind: ButtonKind::Primary,
                                            disabled: !can_save,
                                            onclick: {
                                                let mut save = save.clone();
                                                move |_| save()
                                            },
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}

fn variables_summary(text: &str) -> String {
    match forge_domain::parse_env(text) {
        Ok(list) if list.is_empty() => tr!("None in this environment").to_string(),
        Ok(list) => tr!("{count} variables", count = list.len()),
        Err(_) => tr!("Needs a fix").to_string(),
    }
}

#[component]
fn FolderPopover(search: Signal<String>, taken: Vec<PathBuf>, onpick: EventHandler<PathBuf>, onclose: EventHandler<()>) -> Element {
    let workspace = use_workspace();
    let query = search.read().to_lowercase();
    let known: Vec<PathBuf> = workspace
        .new_session_projects()
        .into_iter()
        .filter(|project| project.exists && !taken.contains(&project.path))
        .map(|project| project.path)
        .filter(|path| query.is_empty() || folder_name(path).to_lowercase().contains(&query))
        .collect();
    rsx! {
        div { class: "picker-menu env-folder-popover", role: "menu",
            input {
                class: "env-search",
                r#type: "text",
                value: "{search}",
                onmounted: move |mounted| async move {
                    let _ = mounted.set_focus(true).await;
                },
                oninput: move |event| search.set(event.value()),
            }
            for path in known {
                button {
                    key: "{path.display()}",
                    class: "picker-item",
                    r#type: "button",
                    title: "{path.display()}",
                    onclick: {
                        let path = path.clone();
                        move |_| onpick.call(path.clone())
                    },
                    span { class: "picker-check" }
                    span { class: "picker-text", span { class: "picker-label", "{folder_name(&path)}" } }
                }
            }
            div { class: "picker-divider" }
            button {
                class: "picker-item",
                r#type: "button",
                onclick: move |_| {
                    spawn(async move {
                        match workspace.pick_folder().await {
                            Some(path) => onpick.call(path),
                            None => onclose.call(()),
                        }
                    });
                },
                span { class: "picker-check", "📁" }
                span { class: "picker-text", span { class: "picker-label", {tr!("Choose…")} } }
            }
        }
    }
}
```

Notes: `branches.read().clone().flatten()` is `Option<BranchList>`; it is `None` both while git runs and for a folder that is not a repository, so the row may show `Not a Git repository` for a moment — if that flash is visible in the window check, show nothing until `branches.finished()`. The select shows the repository's current branch while `draft.branch` is `None`; the stored `branch` stays `None` (the checked-out branch) until the person picks one.

`crates/forge-widgets/src/lib.rs`: `mod environment_dialog;` and `pub use environment_dialog::EnvironmentDialog;` (next to `pub use shortcuts_dialog::ShortcutsDialog;`).

`crates/forge-pages/src/workspace_page.rs`: add `EnvironmentDialog` to the `use forge_widgets::{…}` list and put `EnvironmentDialog {}` on the line after `ShortcutsDialog {}`.

`crates/forge-shared/src/theme.css` — append (reuse the colour variables of the neighbouring dialogs, e.g. `.shortcuts-panel`, `.trust-card`):

```css
.env-dialog { position: relative; width: min(560px, calc(100vw - 32px)); max-height: calc(100vh - 64px); overflow: auto; padding: 20px 24px; border-radius: 16px; background: var(--bg-elevated, var(--bg)); box-shadow: 0 12px 40px rgba(0,0,0,.25); }
.env-head { display: flex; align-items: center; gap: 8px; }
.env-head h2 { flex: 1; margin: 0 0 12px; }
.env-back { border: 0; background: transparent; font-size: 22px; cursor: pointer; color: inherit; }
.env-label { display: block; margin: 14px 0 6px; font-weight: 600; }
.env-folders-head { display: flex; align-items: center; justify-content: space-between; }
.env-add { border: 1px solid var(--border); background: transparent; border-radius: 8px; padding: 4px 10px; cursor: pointer; color: inherit; }
.env-empty { border: 1px dashed var(--border); border-radius: 10px; padding: 14px; color: var(--muted); }
.env-folders { border: 1px solid var(--border); border-radius: 10px; }
.env-folder { display: flex; align-items: center; gap: 10px; padding: 10px 12px; }
.env-folder + .env-folder { border-top: 1px solid var(--border); }
.env-folder-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.env-branch { font-family: var(--mono, monospace); border: 0; background: transparent; color: var(--muted); }
.env-worktree { display: inline-flex; align-items: center; gap: 4px; color: var(--muted); }
.env-note { color: var(--muted); }
.env-remove { visibility: hidden; border: 0; background: transparent; cursor: pointer; color: inherit; }
.env-folder:hover .env-remove { visibility: visible; }
.env-variables-card { display: flex; align-items: center; gap: 10px; width: 100%; margin-top: 14px; padding: 12px 14px; border: 1px solid var(--border); border-radius: 10px; background: transparent; text-align: left; cursor: pointer; color: inherit; }
.env-variables-card .hint { flex: 1; }
.env-variables { width: 100%; min-height: 180px; padding: 10px; border: 1px solid var(--border); border-radius: 10px; font-family: var(--mono, monospace); font-size: 13px; background: transparent; color: inherit; resize: vertical; }
.env-buttons { display: flex; align-items: center; gap: 8px; margin-top: 18px; }
.env-buttons .spacer { flex: 1; }
.env-delete { border: 0; background: transparent; color: #b4281e; cursor: pointer; }
.env-ask-backdrop { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; background: rgba(0,0,0,.2); border-radius: 16px; }
.env-ask { width: min(440px, 90%); padding: 18px 20px; border-radius: 14px; background: var(--bg-elevated, var(--bg)); box-shadow: 0 8px 28px rgba(0,0,0,.3); }
.env-folder-popover { position: absolute; right: 0; z-index: 5; min-width: 260px; }
```

- [ ] **Step 3: Check it compiles and passes**

Run: `cargo build --workspace 2>&1 | tail -20` — Expected: builds after the three corrections above.
Run: `cargo test --workspace 2>&1 | tail -15` — Expected: PASS (nothing existing breaks; the draft and start-plan logic is covered by Task 3's tests).

- [ ] **Step 4: Real-window check (harness)**

`scripts/harness/launch.sh`, then drive with `capture.py` / `click.py` (the harness fixture folders are under `/private/tmp/forge-harness-home`; use two of its folders):
- ⌥⌘0 (or the `+ New environment` row) opens `New environment`; the name is the folder's name; `Create environment` is enabled; the `×` of a folder row shows on hover.
- Add a second folder with `+ Add` → a known folder; with `Choose…` a folder not on the trusted list shows the trust question; `Cancel` does not add it.
- `Variables` card → type `A=1` then a bad line `oops`: the red line text shows and `Done` is disabled; fix it, `Done`; the card reads `1 variables`.
- Make it. The chip menu now lists it as `⌥⌘2`; hovering shows ⚙ and ☆; ☆ pins it (the star fills); ⚙ opens `Edit environment` with `Delete environment` at the left; closing a changed dialog asks `Save the changes?`.
- Pick it: the chip shows its name and the folder, branch and add-folder chips disappear. Pick `This computer`: they return.
- Delete it from its edit dialog while it is picked: the chip returns to `This computer`.
Stop with `scripts/harness/stop.sh`. A real message is sent only in Task 6.

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat: new and edit environment dialog with the variables page

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Korean text, a real session, docs

**Files:**
- Modify: `crates/forge-shared/src/ko.rs` (through `scripts/i18n_add.py`)
- Modify: `docs/superpowers/roadmap/backlog.md` (row OTH-11)
- Modify: `docs/superpowers/specs/2026-10-19-environments-design.md` (an "As built" section)

- [ ] **Step 1: Find every new string without a Korean entry**

Run: `cargo test -p forge-shared 2>&1 | tail -15`
Expected: the repo's i18n test (it checks that every `tr!`/`msg!` key has a Korean entry; if it does not exist in this repo, search by hand: `grep -rhoE 'tr!\("[^"]+"|msg!\("[^"]+"' crates/forge-widgets/src/environment_chip.rs crates/forge-widgets/src/environment_dialog.rs crates/forge-features/src/environment.rs crates/forge-features/src/new_session.rs crates/forge-features/src/shortcut.rs crates/forge-features/src/workspace.rs | sort -u`).

- [ ] **Step 2: Add the Korean entries**

Run from the repo root (text taken from the desktop app where it was seen; the rest follows its tone). A key already present stops the script with `already there:` — then drop that pair and run again:

```bash
python3 scripts/i18n_add.py \
 'This computer' '이 컴퓨터' \
 'Environment' '환경' \
 'Search environments…' '환경 검색…' \
 'More' '더 보기' \
 'New environment' '새 환경' \
 'Edit environment' '환경 편집' \
 'Create environment' '환경 만들기' \
 'Delete environment' '환경 삭제' \
 'Delete environment?' '환경을 삭제하시겠습니까?' \
 '“{name}” will be deleted for good. Sessions started from it are kept.' '“{name}” 환경이 영구적으로 삭제됩니다. 이 환경에서 시작된 세션은 유지됩니다.' \
 'Create this environment?' '이 환경을 만드시겠습니까?' \
 'Save the changes?' '변경 내용을 저장하시겠습니까?' \
 'Discard' '삭제' \
 'Keep editing' '계속 편집' \
 'Name' '이름' \
 'Folders' '폴더' \
 '(optional)' '(선택 사항)' \
 '+ Add' '+ 추가' \
 'Choose…' '선택…' \
 'Choose a folder to start the session in.' '세션을 시작할 폴더를 선택하세요.' \
 'Not a Git repository' 'Git 저장소가 아님' \
 'Variables' '환경 변수' \
 'None in this environment' '이 환경에는 없음' \
 '{count} variables' '변수 {count}개' \
 'Needs a fix' '수정이 필요합니다' \
 'Set for sessions started from this environment.' '이 환경에서 시작된 세션에 대해 설정됩니다.' \
 "Saved in Forge's settings file on this computer. Written in .env format and passed to Claude sessions started from this environment." 'Forge 설정 파일에 이 컴퓨터에만 저장됩니다. .env 형식으로 작성하며 이 환경에서 시작한 Claude 세션에 전달됩니다.' \
 'Line {line}: expected NAME=value' '{line}번째 줄: NAME=값 형식이어야 합니다' \
 'Line {line}: the name may use letters, digits and _ and must not start with a digit' '{line}번째 줄: 이름에는 영문, 숫자, _만 쓸 수 있고 숫자로 시작할 수 없습니다' \
 'Line {line}: a quote is not closed' '{line}번째 줄: 따옴표가 닫히지 않았습니다' \
 'Add a folder to this environment first' '먼저 이 환경에 폴더를 추가하세요' \
 'The variables of this environment are not valid (line {line}). Edit the environment.' '이 환경의 변수가 올바르지 않습니다({line}번째 줄). 환경을 편집하세요.' \
 'Could not save the environment: {error}' '환경을 저장하지 못했습니다: {error}' \
 'Pick an environment 1…9' '환경 선택 1…9' \
 'Unpin' '고정 해제' \
 'Pin' '고정' \
 'Remove' '제거' \
 'Back' '뒤로' \
 'Done' '완료'
```

(`Local`, `Cancel`, `Save`, `Worktree`, `Close (Esc)`, `Trust and add`, `Allow Claude to use this folder and its subfolders?` and `Could not save the trusted folder: {error}` already exist and are not listed. If another key in the list already exists, the script stops with `already there:`; drop that pair and run it again.)

Run: `cargo test -p forge-shared 2>&1 | tail -6` — Expected: PASS.

- [ ] **Step 3: Real session with a variable and a second folder (harness, one real message)**

Ask the user for permission before sending (it costs a real turn on their account), then in the harness: make an environment `obs` with two folders from the harness fixture and the variable `FORGE_ENV_CHECK=hello`, pick it, send `Run: echo $FORGE_ENV_CHECK and list the second folder with ls`, with the permission mode that lets Bash run (or approve the card). Verify: the output shows `hello`; the second folder is readable without an extra permission question about the folder; the session's transcript path is under the first folder. Record what was seen (including anything unexpected) in the spec's `As built` section.

- [ ] **Step 4: Backlog and spec notes**

`docs/superpowers/roadmap/backlog.md`: add the row after `OTH-10`:

```
| OTH-11 | Local environments: a chip on the new-session screen with a menu (search, `이 컴퓨터` ⌥⌘1, saved ones ⌥⌘2…, `더 보기`, `새 환경` ⌥⌘0, hover gear and star), a `새 환경` / `환경 편집` dialog (name, folders, the first folder's branch and worktree, `.env` variables, delete), a session that starts in the first folder with the others as added directories and the variables in its environment | seen | has | L | Plan 53 stage A (built) | Round 21 (parts 1 to 3). Local only: no cloud, SSH, Slack, Remote Control tab, switch or setting; no sidebar `환경` filter. Not built: the `환경` side-panel card and the `시작 위치 … 환경으로 저장` line (stage B), model and effort in an environment, seeding from past sessions, encrypted storage (variables are plain text in `config.json`). |
```

`docs/superpowers/specs/2026-10-19-environments-design.md`: append `## As built (2026-10-19)` with: what differs from the spec (a dedicated `EnvironmentChip` instead of extending `Picker`; trust is asked when a folder is added to the dialog, not at save; `EnvVars` in `forge-domain`; `parse_env` in `forge-domain` not `forge-entities`), the results of Step 3, and what is left.

- [ ] **Step 5: Run everything and commit**

Run: `cargo test --workspace 2>&1 | tail -15` and `(cd sidecar && npx vitest run) 2>&1 | tail -6` — Expected: PASS.

```bash
git add -A
git commit -m "docs: environments built (Plan 53 stage A), Korean text

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

## Self-review (against the spec)

- Goal and success criteria: chip and menu (Tasks 3, 4), dialogs (5), start with first folder, extra directories and variables (2, 3), keys (3).
- Facts and decisions: no cloud/SSH/Remote Control anywhere (Global Constraints); menu order (Task 3 tests); one worktree choice on the first folder (Task 1 `normalized`, Task 3 `start_plan`); no model/effort stored (Global Constraints); plain-text storage notice (Task 5 note, Task 6 Korean text).
- Open facts of the spec: SDK `env` semantics — handled by the explicit `{ ...process.env, ...command.env }` and the vitest test; the visible-row rule and the star are checked in the window (Task 4/5 checks); the permission-mode question needs no code (the draft's mode is left alone); ⌥⌘ conflicts — the table test in Task 3 and the matcher change are covered by `shortcut_for`'s test and the existing duplicate-key tests.
- Deviations from the spec are listed in Task 6 Step 4 (`As built`).
