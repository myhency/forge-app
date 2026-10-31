# Forge Sessions Keep Their Own Folder Implementation Plan (Plan 12)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Folders whose paths differ only in characters Claude replaces with `-` (`/work/my_app`, `/work/my-app`) are shown as separate projects, and a session opened or started under one always runs in that folder.

**Architecture:** Each transcript carries its own `cwd`. One function in the Claude file adapter assigns every transcript of a directory to a `cwd`; `list_projects` emits one project per `cwd`, and `list_sessions` keeps only the transcripts of the folder asked for. Nothing above the adapters changes except the sidebar label for same-named folders. Resuming already starts the sidecar with the project's path, and a missing folder is already view-only.

**Tech Stack:** Rust workspace (tokio, Dioxus 0.7.10). No sidecar change, no port signature change.

**Spec:** `docs/superpowers/specs/2026-09-27-forge-design.md` §23 (follow-up recorded in §15.12).

## Global Constraints

- Commit messages end with the line `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`; subject style `feat: ...`, `fix: ...`, `docs: ...`, lower case.
- A project is a pair of Claude directory and `cwd`; `ProjectSummary.path` is the `cwd` as written in the group's newest transcript (spec §23.1, §23.2).
- A transcript's `cwd` is the first non-empty `cwd` in its first `MAX_CWD_SCAN_LINES` (60) lines; a transcript with none takes the `cwd` of the newest transcript of that directory that has one (spec §23.2).
- Two `cwd` strings are the same folder when their resolved paths are equal; an unresolvable path is compared as written (spec §23.2).
- A directory in which no transcript names a `cwd`: `list_projects` skips it, `list_sessions` lists all of its transcripts (spec §23.2, §23.3).
- `list_projects` keeps a group only if `project_dir(group path)` is the directory it was found in (the existing round-trip rule, spec §23.3).
- Port signatures of `SessionRepositoryPort` and `ProjectRepositoryPort` do not change; `ResumeSessionUseCase` is not touched (spec §23.1, §23.4).
- Labels: same-named projects become `name (parent)` with the nearest parent component that tells them apart; a unique name stays plain; the tooltip keeps the full path (spec §23.5).
- Run commands from the worktree root `/Users/james/orca/workspaces/forge/plan-12`. Record the baseline first: `cargo test --workspace 2>&1 | grep "test result"` and note the totals (Plan 11 part 2 ended at 472+ Rust tests; the exact baseline is whatever this prints).

## Review Focus

1. Two folders in one directory: each project lists only its own sessions and counts, in both `list_projects` and `list_sessions`, and the two methods agree (Tasks 1 and 2 tests).
2. A transcript with no `cwd` among transcripts that have one: it stays visible, under the newest group, never lost or duplicated (Tasks 1 and 2 tests).
3. A directory where no transcript names a `cwd`: `list_sessions` still lists everything, `list_projects` still skips it (Task 2 test plus the existing tests).
4. One folder written two ways in transcripts (`/x` and `/x/.`, a symlink): one project, not two (Task 1 test).
5. A stray `cwd` that does not map back to the directory (for example a transcript moved by hand): that group is dropped, the others stay (existing round-trip test, kept passing in Task 1).

---

### Task 1: One project per `cwd` (`list_projects`)

**Files:**
- Create: `crates/forge-adapter-claude-fs/src/transcript_cwd.rs`
- Modify: `crates/forge-adapter-claude-fs/src/lib.rs` (add `mod transcript_cwd;`)
- Modify: `crates/forge-adapter-claude-fs/src/projects.rs` (use the new module, group by `cwd`; remove `recover_cwd`, `read_cwd`, `transcripts_newest_first`, `MAX_CWD_SCAN_LINES`)
- Test: the `tests` modules of both files

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces (all `pub(crate)` in `transcript_cwd.rs`, used by Task 2):
  - `struct Transcript { modified: SystemTime, path: PathBuf, cwd: PathBuf, key: PathBuf }`
  - `async fn transcripts_newest_first(dir: &Path) -> Vec<(SystemTime, PathBuf)>`
  - `async fn assign_cwds(transcripts: Vec<(SystemTime, PathBuf)>) -> Vec<Transcript>` (input and output newest first; empty when no transcript names a `cwd`)
  - `async fn folder_key(path: &Path) -> PathBuf` (resolved path, or the path as written if it cannot be resolved)

- [ ] **Step 1: Create `transcript_cwd.rs` with the moved functions and the new tests (tests first, bodies stubbed)**

Create `crates/forge-adapter-claude-fs/src/transcript_cwd.rs`. The two moved functions are copied verbatim from `projects.rs` (the bodies below are those bodies); `assign_cwds` and `folder_key` start as `todo!()` so the tests fail first.

```rust
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};

// How many lines of one transcript are searched for a `cwd`. Real transcripts carry one within
// their first few lines; the bound only stops a pathological file from being read in full.
const MAX_CWD_SCAN_LINES: usize = 60;

/// One transcript and the folder it belongs to.
pub(crate) struct Transcript {
    pub modified: SystemTime,
    pub path: PathBuf,
    /// The folder as written in the transcript (or inherited, see `assign_cwds`).
    pub cwd: PathBuf,
    /// `cwd` with symlinks resolved; two transcripts are of the same folder when their keys match.
    pub key: PathBuf,
}

// Every `*.jsonl` file in `dir` with its modification time, newest first (ties broken by path so
// the order is deterministic). Unreadable entries are skipped, never an error.
pub(crate) async fn transcripts_newest_first(dir: &Path) -> Vec<(SystemTime, PathBuf)> {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return Vec::new();
    };
    let mut transcripts = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(metadata) = tokio::fs::metadata(&path).await else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        transcripts.push((modified, path));
    }
    transcripts.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    transcripts
}

// The first non-empty `cwd` string in the first `MAX_CWD_SCAN_LINES` lines. A line that is not
// valid JSON (a partial final line of a transcript still being appended to, or invalid UTF-8) is
// skipped rather than failing the scan.
async fn read_cwd(path: &Path) -> Option<PathBuf> {
    let file = tokio::fs::File::open(path).await.ok()?;
    let mut lines = BufReader::new(file).split(b'\n');
    for _ in 0..MAX_CWD_SCAN_LINES {
        let Ok(Some(line)) = lines.next_segment().await else {
            return None;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&line) else {
            continue;
        };
        if let Some(cwd) = value
            .get("cwd")
            .and_then(Value::as_str)
            .filter(|cwd| !cwd.is_empty())
        {
            return Some(PathBuf::from(cwd));
        }
    }
    None
}

/// The path with symlinks, `.` and a trailing `/` resolved; a path that cannot be resolved (a
/// folder that no longer exists) is used as written.
pub(crate) async fn folder_key(path: &Path) -> PathBuf {
    todo!()
}

/// Gives every transcript of one directory the folder it belongs to. `transcripts` must be newest
/// first; so is the result. A transcript that names no `cwd` takes the `cwd` of the newest
/// transcript that does. When no transcript names one, nothing is assigned (empty result).
pub(crate) async fn assign_cwds(transcripts: Vec<(SystemTime, PathBuf)>) -> Vec<Transcript> {
    todo!()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn cwd_line(cwd: &Path) -> String {
        serde_json::json!({ "type": "user", "cwd": cwd.to_string_lossy() }).to_string()
    }

    fn file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[tokio::test]
    async fn each_transcript_gets_the_cwd_it_names() {
        let dir = tempfile::tempdir().unwrap();
        let a = file(dir.path(), "a.jsonl", &cwd_line(Path::new("/work/a")));
        let b = file(dir.path(), "b.jsonl", &cwd_line(Path::new("/work/b")));

        let assigned = assign_cwds(vec![(at(2), b.clone()), (at(1), a.clone())]).await;

        assert_eq!(assigned.len(), 2);
        assert_eq!((assigned[0].path.clone(), assigned[0].cwd.clone()), (b, PathBuf::from("/work/b")));
        assert_eq!((assigned[1].path.clone(), assigned[1].cwd.clone()), (a, PathBuf::from("/work/a")));
    }

    #[tokio::test]
    async fn a_transcript_with_no_cwd_takes_the_cwd_of_the_newest_transcript_that_has_one() {
        let dir = tempfile::tempdir().unwrap();
        let a = file(dir.path(), "a.jsonl", &cwd_line(Path::new("/work/a")));
        let b = file(dir.path(), "b.jsonl", &cwd_line(Path::new("/work/b")));
        let bare = file(dir.path(), "bare.jsonl", r#"{"type":"attachment"}"#);

        let assigned = assign_cwds(vec![(at(3), bare), (at(2), b), (at(1), a)]).await;

        assert_eq!(assigned[0].cwd, PathBuf::from("/work/b"));
        assert_eq!(assigned[2].cwd, PathBuf::from("/work/a"));
    }

    #[tokio::test]
    async fn nothing_is_assigned_when_no_transcript_names_a_cwd() {
        let dir = tempfile::tempdir().unwrap();
        let bare = file(dir.path(), "bare.jsonl", r#"{"type":"attachment"}"#);

        assert!(assign_cwds(vec![(at(1), bare)]).await.is_empty());
    }

    #[tokio::test]
    async fn two_spellings_of_one_folder_share_a_key() {
        let folder = tempfile::tempdir().unwrap();
        let real = folder.path().canonicalize().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let plain = file(dir.path(), "plain.jsonl", &cwd_line(&real));
        let dotted = file(dir.path(), "dotted.jsonl", &cwd_line(&real.join(".")));

        let assigned = assign_cwds(vec![(at(2), dotted), (at(1), plain)]).await;

        assert_ne!(assigned[0].cwd, assigned[1].cwd, "the cwd keeps the spelling it was written in");
        assert_eq!(assigned[0].key, assigned[1].key);
    }

    #[tokio::test]
    async fn a_folder_that_no_longer_exists_is_keyed_as_written() {
        assert_eq!(folder_key(Path::new("/gone/for/good")).await, PathBuf::from("/gone/for/good"));
    }
}
```

Add `mod transcript_cwd;` to `crates/forge-adapter-claude-fs/src/lib.rs`, after `mod projects;`. In `projects.rs`, delete `MAX_CWD_SCAN_LINES`, `recover_cwd`, `read_cwd` and `transcripts_newest_first` (the bodies now live in `transcript_cwd.rs`), and import what remains in use:

```rust
use crate::transcript_cwd::{assign_cwds, transcripts_newest_first};
```

Leave the body of `list_projects` untouched for now except that it no longer compiles without `recover_cwd`; temporarily keep it compiling by editing only this one line, so the failing run in Step 2 is about the new tests:

```rust
            let Some(path) = assign_cwds(transcripts.clone()).await.first().map(|t| t.cwd.clone()) else {
                continue;
            };
```

- [ ] **Step 2: Run the new tests to verify they fail**

Run: `cargo test -p forge-adapter-claude-fs transcript_cwd 2>&1 | tail -20`
Expected: the five tests FAIL with `not yet implemented` (the `todo!()` panics).

- [ ] **Step 3: Implement `folder_key` and `assign_cwds`**

Replace the two `todo!()` bodies in `transcript_cwd.rs`:

```rust
pub(crate) async fn folder_key(path: &Path) -> PathBuf {
    tokio::fs::canonicalize(path)
        .await
        .unwrap_or_else(|_| path.to_path_buf())
}

pub(crate) async fn assign_cwds(transcripts: Vec<(SystemTime, PathBuf)>) -> Vec<Transcript> {
    let mut read = Vec::with_capacity(transcripts.len());
    for (modified, path) in transcripts {
        let cwd = read_cwd(&path).await;
        read.push((modified, path, cwd));
    }
    // Newest first, so this is the newest transcript that names a cwd.
    let Some(inherited) = read.iter().find_map(|(_, _, cwd)| cwd.clone()) else {
        return Vec::new();
    };
    let mut assigned = Vec::with_capacity(read.len());
    for (modified, path, cwd) in read {
        let cwd = cwd.unwrap_or_else(|| inherited.clone());
        let key = folder_key(&cwd).await;
        assigned.push(Transcript { modified, path, cwd, key });
    }
    assigned
}
```

- [ ] **Step 4: Run the new tests to verify they pass**

Run: `cargo test -p forge-adapter-claude-fs transcript_cwd 2>&1 | tail -20`
Expected: 5 passed.

- [ ] **Step 5: Commit the module on its own**

```bash
git add crates/forge-adapter-claude-fs/src/transcript_cwd.rs crates/forge-adapter-claude-fs/src/lib.rs crates/forge-adapter-claude-fs/src/projects.rs
git commit -m "feat: assign every transcript of a Claude directory to the folder it names" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 6: Write the failing `list_projects` tests**

Add to the `tests` module of `projects.rs` (it already has `at`, `cwd_line`, `add_transcript`, `write_transcript_in`, `list`). `/work/my_app` and `/work/my-app` both escape to `-work-my-app`, so `add_transcript` puts both in one directory.

```rust
    #[tokio::test]
    async fn list_projects_lists_two_folders_sharing_one_claude_directory_as_two_projects() {
        let home = tempfile::tempdir().unwrap();
        let first = Path::new("/work/my_app");
        let second = Path::new("/work/my-app");
        add_transcript(home.path(), first, "a-1", cwd_line(first).as_bytes(), at(100)).await;
        add_transcript(home.path(), first, "a-2", cwd_line(first).as_bytes(), at(300)).await;
        add_transcript(home.path(), second, "b-1", cwd_line(second).as_bytes(), at(200)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].path, PathBuf::from("/work/my_app"), "newest activity first");
        assert_eq!(projects[0].session_count, 2);
        assert_eq!(projects[1].path, PathBuf::from("/work/my-app"));
        assert_eq!(projects[1].session_count, 1);
    }

    #[tokio::test]
    async fn list_projects_counts_a_transcript_with_no_cwd_under_the_newest_group() {
        let home = tempfile::tempdir().unwrap();
        let first = Path::new("/work/my_app");
        let second = Path::new("/work/my-app");
        add_transcript(home.path(), first, "a-1", cwd_line(first).as_bytes(), at(100)).await;
        add_transcript(home.path(), second, "b-1", cwd_line(second).as_bytes(), at(200)).await;
        add_transcript(home.path(), second, "bare", br#"{"type":"attachment"}"#, at(300)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].path, PathBuf::from("/work/my-app"));
        assert_eq!(projects[0].session_count, 2);
        assert_eq!(projects[1].path, PathBuf::from("/work/my_app"));
        assert_eq!(projects[1].session_count, 1);
    }

    #[tokio::test]
    async fn list_projects_merges_two_spellings_of_one_folder() {
        let home = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let real = folder.path().canonicalize().unwrap();
        add_transcript(home.path(), &real, "plain", cwd_line(&real).as_bytes(), at(200)).await;
        add_transcript(home.path(), &real, "dotted", cwd_line(&real.join(".")).as_bytes(), at(100)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, real);
        assert_eq!(projects[0].session_count, 2);
    }

    #[tokio::test]
    async fn list_projects_drops_a_stray_group_but_keeps_the_others() {
        let home = tempfile::tempdir().unwrap();
        let project = Path::new("/work/a");
        add_transcript(home.path(), project, "good", cwd_line(project).as_bytes(), at(200)).await;
        add_transcript(home.path(), project, "stray", cwd_line(Path::new("/elsewhere")).as_bytes(), at(300)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, PathBuf::from("/work/a"));
        assert_eq!(projects[0].session_count, 1);
    }
```

- [ ] **Step 7: Run them to verify they fail**

Run: `cargo test -p forge-adapter-claude-fs list_projects 2>&1 | tail -30`
Expected: the four new tests FAIL (one project instead of two; `session_count` wrong; the stray test fails because the newest transcript's `cwd` is `/elsewhere`, which does not map back, so the whole directory is skipped today). All older `list_projects` tests still pass.

- [ ] **Step 8: Group by `cwd` in `list_projects`**

Replace the part of the loop body in `projects.rs` that starts at `let transcripts = transcripts_newest_first(&dir).await;` and ends at the `found.push(...)` call, with:

```rust
            let assigned = assign_cwds(transcripts_newest_first(&dir).await).await;

            // One project per folder (spec §23): transcripts are newest first, so a group's first
            // member is its newest and decides `path` and `last_active`.
            let mut groups: Vec<(&PathBuf, Vec<&Transcript>)> = Vec::new();
            for transcript in &assigned {
                match groups.iter_mut().find(|(key, _)| **key == transcript.key) {
                    Some((_, members)) => members.push(transcript),
                    None => groups.push((&transcript.key, vec![transcript])),
                }
            }

            for (_, members) in groups {
                let newest = members[0];
                let path = newest.cwd.clone();
                let Ok(last_active) = time::OffsetDateTime::from(newest.modified).format(&Rfc3339) else {
                    continue;
                };

                // Keep a group only if `ClaudeSessionAdapter` finds its directory again from the
                // group's path, so `list_sessions(project.path)` lists exactly this folder.
                if sessions.project_dir(&path).await != dir {
                    continue;
                }

                let exists = tokio::fs::metadata(&path)
                    .await
                    .map(|m| m.is_dir())
                    .unwrap_or(false);
                found.push((
                    newest.modified,
                    ProjectSummary {
                        path,
                        last_active,
                        session_count: members.len(),
                        exists,
                    },
                ));
            }
```

Update the import to `use crate::transcript_cwd::{assign_cwds, transcripts_newest_first, Transcript};`. Remove `Path` from the top `use std::path::...` if the compiler reports it unused (the tests module has its own import).

- [ ] **Step 9: Run the whole adapter crate**

Run: `cargo test -p forge-adapter-claude-fs 2>&1 | grep -E "test result|FAILED|failed"`
Expected: all pass, including the older `list_projects` tests (`..._uses_the_newest_transcripts_cwd_when_transcripts_disagree` keeps passing: its older `/work/b` group is dropped because it does not map back to the directory).

- [ ] **Step 10: Commit**

```bash
git add crates/forge-adapter-claude-fs/src/projects.rs
git commit -m "feat: one project per folder when folders share a Claude directory" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: `list_sessions` lists only the folder asked for

**Files:**
- Modify: `crates/forge-adapter-claude-fs/src/sessions.rs` (`list_sessions`, imports)
- Test: `tests` modules of `sessions.rs` and `projects.rs`

**Interfaces:**
- Consumes: `assign_cwds`, `folder_key`, `transcripts_newest_first`, `Transcript` from Task 1 (`crate::transcript_cwd`).
- Produces: `list_sessions(project_path)` returns only transcripts whose assigned `cwd` is the same folder as `project_path`, or every transcript when no transcript in the directory names a `cwd`.

- [ ] **Step 1: Write the failing tests**

In the `tests` module of `sessions.rs` (it has `write_session(home, project, id, lines) -> PathBuf`, `set_mtime(path, secs)`, `user_text`, `json!`), add:

```rust
    fn cwd_user(cwd: &str, text: &str) -> Value {
        json!({ "type": "user", "cwd": cwd, "message": { "role": "user", "content": text } })
    }

    fn ids(sessions: &[SessionSummary]) -> Vec<&str> {
        sessions.iter().map(|s| s.id.0.as_str()).collect()
    }

    #[tokio::test]
    async fn list_sessions_lists_only_the_sessions_of_the_folder_asked_for() {
        let home = tempfile::tempdir().unwrap();
        write_session(home.path(), Path::new("/work/my_app"), "a-1", &[cwd_user("/work/my_app", "one")]);
        write_session(home.path(), Path::new("/work/my-app"), "b-1", &[cwd_user("/work/my-app", "two")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let first = adapter.list_sessions(Path::new("/work/my_app")).await.unwrap();
        let second = adapter.list_sessions(Path::new("/work/my-app")).await.unwrap();

        assert_eq!(ids(&first), ["a-1"]);
        assert_eq!(ids(&second), ["b-1"]);
    }

    #[tokio::test]
    async fn list_sessions_gives_a_transcript_with_no_cwd_to_the_newest_folder() {
        let home = tempfile::tempdir().unwrap();
        let a = write_session(home.path(), Path::new("/work/my_app"), "a-1", &[cwd_user("/work/my_app", "one")]);
        let b = write_session(home.path(), Path::new("/work/my-app"), "b-1", &[cwd_user("/work/my-app", "two")]);
        let bare = write_session(home.path(), Path::new("/work/my-app"), "bare", &[json!({ "type": "attachment" })]);
        set_mtime(&a, 100);
        set_mtime(&b, 200);
        set_mtime(&bare, 300);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let first = adapter.list_sessions(Path::new("/work/my_app")).await.unwrap();
        let second = adapter.list_sessions(Path::new("/work/my-app")).await.unwrap();

        assert_eq!(ids(&first), ["a-1"]);
        assert_eq!(ids(&second), ["bare", "b-1"]);
    }

    #[tokio::test]
    async fn list_sessions_for_a_folder_no_transcript_names_lists_nothing() {
        let home = tempfile::tempdir().unwrap();
        write_session(home.path(), Path::new("/work/my_app"), "a-1", &[cwd_user("/work/my_app", "one")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let sessions = adapter.list_sessions(Path::new("/work/my-app")).await.unwrap();

        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn list_sessions_does_not_split_a_directory_where_no_transcript_names_a_cwd() {
        let home = tempfile::tempdir().unwrap();
        write_session(home.path(), &project(), "s-1", &[user_text("one")]);
        write_session(home.path(), &project(), "s-2", &[user_text("two")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let sessions = adapter.list_sessions(&project()).await.unwrap();

        assert_eq!(sessions.len(), 2);
    }

    #[tokio::test]
    async fn list_sessions_matches_a_folder_written_with_a_different_spelling() {
        let home = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let real = folder.path().canonicalize().unwrap();
        write_session(home.path(), &real, "s-1", &[cwd_user(&real.join(".").to_string_lossy(), "one")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let sessions = adapter.list_sessions(&real).await.unwrap();

        assert_eq!(ids(&sessions), ["s-1"]);
    }
```

Check that `SessionSummary` is imported in the tests module (it is through `use super::*`, since the file imports it at the top). If `write_session` names its directory with `escape_project_path(project)` of an un-canonicalized path (it does), the last test relies on `/tmp` style paths being canonical; `folder.path().canonicalize()` makes it so.

In the `tests` module of `projects.rs`, add the round trip over two folders:

```rust
    #[tokio::test]
    async fn list_projects_round_trips_with_list_sessions_for_two_folders_in_one_directory() {
        let home = tempfile::tempdir().unwrap();
        let first = Path::new("/work/my_app");
        let second = Path::new("/work/my-app");
        add_transcript(home.path(), first, "a-1", cwd_line(first).as_bytes(), at(100)).await;
        add_transcript(home.path(), first, "a-2", cwd_line(first).as_bytes(), at(300)).await;
        add_transcript(home.path(), second, "b-1", cwd_line(second).as_bytes(), at(200)).await;
        add_transcript(home.path(), second, "bare", br#"{"type":"attachment"}"#, at(250)).await;

        let projects = list(home.path()).await.unwrap();
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        assert_eq!(projects.len(), 2);
        for project in &projects {
            let sessions = adapter.list_sessions(&project.path).await.unwrap();
            assert_eq!(sessions.len(), project.session_count, "{}", project.path.display());
        }
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p forge-adapter-claude-fs list_sessions 2>&1 | tail -30`
Expected: `..._only_the_sessions_of_the_folder_asked_for`, `..._gives_a_transcript_with_no_cwd_to_the_newest_folder`, `..._for_a_folder_no_transcript_names_lists_nothing` and the round-trip test FAIL (every transcript of the directory is listed). `..._does_not_split_...` and `..._different_spelling` already pass.

- [ ] **Step 3: Filter in `list_sessions`**

In `sessions.rs` add the imports:

```rust
use std::collections::HashSet;

use crate::transcript_cwd::{assign_cwds, folder_key, transcripts_newest_first};
```

In `list_sessions`, right after the `let mut entries = match tokio::fs::read_dir(&dir).await { ... };` block and before `let mut found`, add:

```rust
        // Folders whose paths differ only in characters Claude replaces share this directory
        // (spec §23): keep only the transcripts of `project_path`'s own folder. A directory in
        // which no transcript names a `cwd` is not split.
        let owners = assign_cwds(transcripts_newest_first(&dir).await).await;
        let wanted = folder_key(project_path).await;
        let owned: Option<HashSet<PathBuf>> = if owners.is_empty() {
            None
        } else {
            Some(owners.into_iter().filter(|t| t.key == wanted).map(|t| t.path).collect())
        };
```

and inside the `while let` loop, right after the `let Some(id) = ... else { continue; };` statement (the `path` binding is already in scope above it), add:

```rust
            if let Some(owned) = &owned {
                if !owned.contains(&path) {
                    continue;
                }
            }
```

- [ ] **Step 4: Run the whole adapter crate**

Run: `cargo test -p forge-adapter-claude-fs 2>&1 | grep -E "test result|FAILED|failed"`
Expected: all pass (the older `list_sessions_*` tests write transcripts without a `cwd`, so they take the "not split" path).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-adapter-claude-fs/src/sessions.rs crates/forge-adapter-claude-fs/src/projects.rs
git commit -m "feat: list_sessions lists only the sessions of the folder it is asked for" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Sidebar labels for same-named folders

**Files:**
- Modify: `crates/forge-widgets/src/sidebar.rs` (add `project_labels`, use it in `Sidebar`)
- Modify: `crates/forge-widgets/src/lib.rs` (re-export `project_labels`)
- Test: `tests` module of `sidebar.rs`

**Interfaces:**
- Consumes: `project_label(path: &Path) -> String` (existing, unchanged; `chat_window.rs` keeps using it).
- Produces: `pub fn project_labels(paths: &[PathBuf]) -> Vec<String>`: one label per path, same order. A name that is unique in the list stays plain; a name shared by several becomes `name (parent)` with the nearest ancestor name that differs across all those paths (the root is written `/`); if no ancestor does, `name (full path)`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module of `sidebar.rs` (add `use std::path::PathBuf;` next to `use std::path::Path;`):

```rust
    fn labels(paths: &[&str]) -> Vec<String> {
        let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        project_labels(&paths)
    }

    #[test]
    fn unique_names_stay_plain() {
        assert_eq!(labels(&["/work/forge", "/work/other"]), ["forge", "other"]);
    }

    #[test]
    fn same_named_folders_show_their_parent() {
        assert_eq!(labels(&["/a/app", "/b/app"]), ["app (a)", "app (b)"]);
    }

    #[test]
    fn the_nearest_parent_that_tells_them_apart_is_used() {
        assert_eq!(labels(&["/x/same/app", "/y/same/app"]), ["app (x)", "app (y)"]);
    }

    #[test]
    fn only_the_shared_names_get_a_parent() {
        assert_eq!(
            labels(&["/a/app", "/work/other", "/b/app"]),
            ["app (a)", "other", "app (b)"]
        );
    }

    #[test]
    fn a_folder_at_the_root_is_told_apart_by_the_root() {
        assert_eq!(labels(&["/app", "/b/app"]), ["app (/)", "app (b)"]);
    }

    #[test]
    fn three_folders_with_one_name_each_get_their_own_parent() {
        assert_eq!(
            labels(&["/a/app", "/b/app", "/c/app"]),
            ["app (a)", "app (b)", "app (c)"]
        );
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p forge-widgets labels 2>&1 | tail -15`
Expected: compile error `cannot find function project_labels`.

- [ ] **Step 3: Implement `project_labels`**

In `sidebar.rs`, below `project_label`:

```rust
// The name of the `depth`-th ancestor of `path` (depth 1 is the parent). The root is written as
// `/`, which keeps `/app` apart from `/b/app`.
fn ancestor_name(path: &Path, depth: usize) -> Option<String> {
    path.ancestors().nth(depth).map(|ancestor| {
        ancestor
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| ancestor.to_string_lossy().to_string())
    })
}

/// One heading per project, in the order of `paths`. Folders that share a name are told apart by
/// the nearest parent name that differs among them: `app (a)` and `app (b)` for `/a/app` and
/// `/b/app`. A name nobody else uses stays plain.
pub fn project_labels(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let base = project_label(path);
            let others: Vec<&PathBuf> = paths
                .iter()
                .enumerate()
                .filter(|(other, candidate)| *other != index && project_label(candidate) == base)
                .map(|(_, candidate)| candidate)
                .collect();
            if others.is_empty() {
                return base;
            }
            let depth_limit = path.ancestors().count();
            for depth in 1..depth_limit {
                let mine = ancestor_name(path, depth);
                if others.iter().all(|other| ancestor_name(other, depth) != mine) {
                    if let Some(parent) = mine {
                        return format!("{base} ({parent})");
                    }
                }
            }
            format!("{base} ({})", path.display())
        })
        .collect()
}
```

Add `use std::path::{Path, PathBuf};` at the top (replacing `use std::path::Path;`), export in `lib.rs`: `pub use sidebar::{project_label, project_labels, Sidebar};`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p forge-widgets labels 2>&1 | tail -15`
Expected: 6 passed. (If `a_folder_at_the_root...` fails, check `Path::new("/app").ancestors()` yields `/app`, `/`, and that depth 1 gives `/`.)

- [ ] **Step 5: Use it in the sidebar**

In `Sidebar`, after `let groups = workspace.sidebar();` add:

```rust
    let labels = project_labels(&groups.iter().map(|group| group.project.path.clone()).collect::<Vec<_>>());
```

Change the loop header `for group in groups {` to `for (group, label) in groups.into_iter().zip(labels) {` and the heading text line `"{project_label(&group.project.path)}"` to `"{label}"`. The `title` attribute (full path) stays.

- [ ] **Step 6: Build and run the widgets tests**

Run: `cargo test -p forge-widgets 2>&1 | grep -E "test result|FAILED|error"`
Expected: all pass, no warnings about an unused `project_label` import (it is still used by `chat_window.rs` and by `project_labels`).

- [ ] **Step 7: Commit**

```bash
git add crates/forge-widgets/src/sidebar.rs crates/forge-widgets/src/lib.rs
git commit -m "feat: sidebar headings tell same-named folders apart by their parent" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Whole-branch check and as-built record

**Files:**
- Modify: `docs/superpowers/specs/2026-09-27-forge-design.md` (append `### 23.8 As built`; mark the §15.12 follow-up closed)

**Interfaces:**
- Consumes: the three earlier tasks.
- Produces: the as-built record the next plan reads.

- [ ] **Step 1: Run the whole workspace**

Run: `cargo test --workspace 2>&1 | grep -E "test result|FAILED|failed"` and `cargo build --workspace 2>&1 | grep -E "^warning|^error" | head`
Expected: all pass; the total is the baseline plus the new tests (5 + 4 + 5 + 1 + 6 = 21 new Rust tests); no new warnings. Note both numbers for Step 3.

- [ ] **Step 2: Check the sidecar tests are untouched**

Run: `git diff main --stat -- sidecar | tail -3`
Expected: no output (this plan changes no sidecar file).

- [ ] **Step 3: Write the as-built section**

Append to the end of the spec, filling the numbers from Step 1 (the baseline and total are whatever Step 1 printed; do not guess them). Cover: the branch (`plan-12`) and the check date (today's date from `date +%F`); the test counts; differences from §23 (the §23.4 correction made while planning, and the `list_sessions` no-`cwd` directory rule added to §23.2/§23.3); and a **Not checked (needs a person)** list with these items: the sidebar with two real folders `/tmp/x/my_app` and `/tmp/x/my-app` that both have sessions (two headings, each listing only its own sessions, opening and sending in each runs `pwd` in its own folder); the label for two same-named folders; sidebar load time with a few hundred transcripts in one directory (spec §23.7 says to measure; no measurement was made unless Step 4 does it).

Also change the §15.12 follow-up line `**Follow-up.**` to `**Follow-up, closed by Plan 12 (§23).**`.

- [ ] **Step 4: Measure the cost named in §23.7 (optional if a real `~/.claude/projects` has a large directory)**

Run: `ls ~/.claude/projects | while read d; do echo "$(ls ~/.claude/projects/$d | wc -l) $d"; done | sort -rn | head -3`
If the largest directory has more than about 200 transcripts, time `list_sessions` on it with a throwaway test or example and record the number in §23.8; otherwise record "largest real directory had N transcripts; no measurement needed".

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/specs/2026-09-27-forge-design.md
git commit -m "docs: record Plan 12 as built" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
