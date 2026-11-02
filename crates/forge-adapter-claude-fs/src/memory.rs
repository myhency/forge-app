use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use forge_ports::{MemoryEntry, MemoryPort, MemoryProject};

use crate::trash::TrashBin;

const INDEX: &str = "MEMORY.md";

/// Claude Code's auto memory on disk: `<claude home>/projects/<folder>/memory/*.md`.
pub struct ClaudeMemoryAdapter {
    claude_home: PathBuf,
    trash: Arc<dyn TrashBin>,
}

impl ClaudeMemoryAdapter {
    pub fn new(claude_home: PathBuf, trash: Arc<dyn TrashBin>) -> Self {
        Self { claude_home, trash }
    }

    fn memory_dir(&self, project: &str) -> Result<PathBuf, String> {
        if !plain_name(project) {
            return Err(format!("not a project folder name: {project}"));
        }
        Ok(self.claude_home.join("projects").join(project).join("memory"))
    }

    fn note_path(&self, project: &str, file: &str) -> Result<PathBuf, String> {
        if !plain_name(file) || !file.ends_with(".md") || file == INDEX {
            return Err(format!("not a memory note: {file}"));
        }
        Ok(self.memory_dir(project)?.join(file))
    }
}

/// A single path component with no separator or dots at the start, so a call cannot leave the folder it names.
fn plain_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\', '\0'])
}

fn seconds(time: SystemTime) -> i64 {
    time.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// The `key: value` lines between the two `---` lines at the top of a note.
fn header(text: &str) -> Vec<(String, String)> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Vec::new();
    }
    let mut found = Vec::new();
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        // Only top-level keys: an indented line belongs to a block such as `metadata:`.
        if line.starts_with(char::is_whitespace) {
            if let Some((key, value)) = line.trim().split_once(':') {
                found.push((key.trim().to_string(), unquote(value.trim())));
            }
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            found.push((key.trim().to_string(), unquote(value.trim())));
        }
    }
    found
}

fn unquote(value: &str) -> String {
    let quoted = value.len() >= 2 && ((value.starts_with('"') && value.ends_with('"')) || (value.starts_with('\'') && value.ends_with('\'')));
    if quoted { value[1..value.len() - 1].to_string() } else { value.to_string() }
}

fn entry_of(path: &Path) -> Option<MemoryEntry> {
    let file = path.file_name()?.to_str()?.to_string();
    if !file.ends_with(".md") || file == INDEX {
        return None;
    }
    let updated = fs::metadata(path).and_then(|m| m.modified()).map(seconds).unwrap_or(0);
    // A note is small; a huge file is not read whole just to find its header.
    let text: String = BufReader::new(fs::File::open(path).ok()?).lines().take(40).map_while(Result::ok).collect::<Vec<_>>().join("\n");
    let fields = header(&text);
    let get = |key: &str| fields.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).unwrap_or_default();
    let name = Some(get("name")).filter(|n| !n.is_empty()).unwrap_or_else(|| file.trim_end_matches(".md").to_string());
    Some(MemoryEntry { file, name, description: get("description"), kind: get("type"), updated })
}

/// The folder a project directory stands for, read from the newest transcript in it; the directory name when no
/// transcript says (the name itself cannot be turned back into a path, as `-` may be a `/` or a real dash).
fn folder_of(dir: &Path) -> Option<String> {
    let mut transcripts: Vec<(SystemTime, PathBuf)> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    transcripts.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path) in transcripts.into_iter().take(3) {
        let Ok(file) = fs::File::open(path) else { continue };
        for line in BufReader::new(file).lines().take(60).map_while(Result::ok) {
            if !line.contains("\"cwd\"") {
                continue;
            }
            if let Some(cwd) = serde_json::from_str::<serde_json::Value>(&line).ok().and_then(|v| v.get("cwd")?.as_str().map(str::to_string)) {
                return Some(cwd);
            }
        }
    }
    None
}

impl MemoryPort for ClaudeMemoryAdapter {
    fn list(&self) -> Result<Vec<MemoryProject>, String> {
        let projects = self.claude_home.join("projects");
        let dirs = match fs::read_dir(&projects) {
            Ok(dirs) => dirs,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("{}: {error}", projects.display())),
        };
        let mut found = Vec::new();
        for dir in dirs.flatten() {
            let Ok(key) = dir.file_name().into_string() else { continue };
            let memory = dir.path().join("memory");
            let Ok(files) = fs::read_dir(&memory) else { continue };
            let mut entries: Vec<MemoryEntry> = files.flatten().filter_map(|f| entry_of(&f.path())).collect();
            if entries.is_empty() {
                continue;
            }
            entries.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| a.file.cmp(&b.file)));
            let path = folder_of(&dir.path()).unwrap_or_else(|| key.clone());
            let label = Path::new(&path).file_name().and_then(|n| n.to_str()).map(str::to_string).unwrap_or_else(|| path.clone());
            found.push(MemoryProject { key, label, path, entries });
        }
        found.sort_by(|a, b| b.entries[0].updated.cmp(&a.entries[0].updated).then_with(|| a.key.cmp(&b.key)));
        Ok(found)
    }

    fn read(&self, project: &str, file: &str) -> Result<String, String> {
        let path = self.note_path(project, file)?;
        fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))
    }

    fn write(&self, project: &str, file: &str, text: &str) -> Result<(), String> {
        let path = self.note_path(project, file)?;
        // Only a note that exists is replaced: this page edits, it does not create.
        if !path.is_file() {
            return Err(format!("{}: no such note", path.display()));
        }
        fs::write(&path, text).map_err(|error| format!("{}: {error}", path.display()))
    }

    fn delete(&self, project: &str, file: &str) -> Result<(), String> {
        let path = self.note_path(project, file)?;
        self.trash.trash(&path)?;
        // The index lists the note with a link to its file; the line goes with it.
        let index = path.with_file_name(INDEX);
        if let Ok(text) = fs::read_to_string(&index) {
            let link = format!("({file})");
            let kept: Vec<&str> = text.lines().filter(|line| !line.contains(&link)).collect();
            if kept.len() != text.lines().count() {
                let mut out = kept.join("\n");
                if text.ends_with('\n') {
                    out.push('\n');
                }
                fs::write(&index, out).map_err(|error| format!("{}: {error}", index.display()))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeTrash(Mutex<Vec<PathBuf>>);
    impl TrashBin for FakeTrash {
        fn trash(&self, path: &Path) -> Result<(), String> {
            self.0.lock().unwrap().push(path.to_path_buf());
            fs::remove_file(path).map_err(|e| e.to_string())
        }
    }

    fn setup() -> (tempfile::TempDir, ClaudeMemoryAdapter) {
        let home = tempfile::tempdir().unwrap();
        let adapter = ClaudeMemoryAdapter::new(home.path().to_path_buf(), Arc::new(FakeTrash::default()));
        (home, adapter)
    }

    fn note(home: &Path, project: &str, file: &str, text: &str) {
        let dir = home.join("projects").join(project).join("memory");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(file), text).unwrap();
    }

    #[test]
    fn a_note_is_listed_with_its_header_and_the_index_is_not() {
        let (home, adapter) = setup();
        note(home.path(), "-tmp-app", "role.md", "---\nname: user-role\ndescription: \"Who the user is\"\nmetadata:\n  type: user\n---\n\nbody\n");
        note(home.path(), "-tmp-app", "MEMORY.md", "- [Role](role.md) — who\n");
        let projects = adapter.list().unwrap();
        assert_eq!(projects.len(), 1);
        let entry = &projects[0].entries[0];
        assert_eq!((entry.file.as_str(), entry.name.as_str(), entry.description.as_str(), entry.kind.as_str()), ("role.md", "user-role", "Who the user is", "user"));
        assert_eq!(projects[0].entries.len(), 1);
    }

    #[test]
    fn a_note_with_no_header_is_named_after_its_file() {
        let (home, adapter) = setup();
        note(home.path(), "p", "plain-note.md", "just text\n");
        let entry = &adapter.list().unwrap()[0].entries[0];
        assert_eq!((entry.name.as_str(), entry.description.as_str()), ("plain-note", ""));
    }

    #[test]
    fn the_folder_comes_from_a_transcript_and_falls_back_to_the_directory_name() {
        let (home, adapter) = setup();
        note(home.path(), "-tmp-my-app", "a.md", "x");
        fs::write(home.path().join("projects/-tmp-my-app/s.jsonl"), "{\"type\":\"summary\"}\n{\"cwd\":\"/tmp/my-app\",\"type\":\"user\"}\n").unwrap();
        note(home.path(), "-tmp-other", "a.md", "x");
        let projects = adapter.list().unwrap();
        let mine = projects.iter().find(|p| p.key == "-tmp-my-app").unwrap();
        assert_eq!((mine.label.as_str(), mine.path.as_str()), ("my-app", "/tmp/my-app"));
        let other = projects.iter().find(|p| p.key == "-tmp-other").unwrap();
        assert_eq!(other.label, "-tmp-other");
    }

    #[test]
    fn a_folder_with_no_notes_is_left_out_and_a_missing_home_is_empty() {
        let (home, adapter) = setup();
        fs::create_dir_all(home.path().join("projects/empty/memory")).unwrap();
        note(home.path(), "only-index", "MEMORY.md", "- x\n");
        assert!(adapter.list().unwrap().is_empty());
        let none = ClaudeMemoryAdapter::new(home.path().join("missing"), Arc::new(FakeTrash::default()));
        assert!(none.list().unwrap().is_empty());
    }

    #[test]
    fn read_and_write_stay_inside_the_memory_directory() {
        let (home, adapter) = setup();
        note(home.path(), "p", "a.md", "old");
        assert_eq!(adapter.read("p", "a.md").unwrap(), "old");
        adapter.write("p", "a.md", "new").unwrap();
        assert_eq!(adapter.read("p", "a.md").unwrap(), "new");
        assert!(adapter.read("p", "../../secret.md").is_err());
        assert!(adapter.read("..", "a.md").is_err());
        assert!(adapter.read("p", "MEMORY.md").is_err());
        assert!(adapter.write("p", "missing.md", "x").is_err(), "editing never creates a note");
    }

    #[test]
    fn deleting_a_note_removes_its_index_line_and_keeps_the_others() {
        let (home, adapter) = setup();
        note(home.path(), "p", "a.md", "x");
        note(home.path(), "p", "b.md", "y");
        note(home.path(), "p", "MEMORY.md", "- [A](a.md) — one\n- [B](b.md) — two\n");
        adapter.delete("p", "a.md").unwrap();
        assert!(adapter.read("p", "a.md").is_err());
        let index = fs::read_to_string(home.path().join("projects/p/memory/MEMORY.md")).unwrap();
        assert_eq!(index, "- [B](b.md) — two\n");
        assert_eq!(adapter.read("p", "b.md").unwrap(), "y");
    }
}
