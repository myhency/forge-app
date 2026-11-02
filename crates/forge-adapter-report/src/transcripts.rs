//! The newest conversations, for a report whose sender chose to include them. A conversation holds what was said, so
//! this is the only part of a report that is opt-in.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::oslogs::Part;

const COUNT: usize = 3;
/// Only the end of a long conversation is taken: that is where a failure is.
const TAIL_BYTES: u64 = 2 * 1024 * 1024;

pub fn newest(projects: Option<&Path>) -> Vec<Part> {
    let Some(projects) = projects else { return Vec::new() };
    let mut found: Vec<(SystemTime, PathBuf)> = std::fs::read_dir(projects)
        .into_iter()
        .flatten()
        .flatten()
        .flat_map(|folder| std::fs::read_dir(folder.path()).into_iter().flatten().flatten())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jsonl"))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found
        .into_iter()
        .take(COUNT)
        .enumerate()
        .filter_map(|(n, (_, path))| Some(Part { name: format!("conversations/{}-{}", n + 1, path.file_name()?.to_string_lossy()), text: tail(&path)? }))
        .collect()
}

/// The last `TAIL_BYTES` of a file, starting at a whole line.
fn tail(path: &Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let cut = len > TAIL_BYTES;
    file.seek(SeekFrom::Start(len.saturating_sub(TAIL_BYTES))).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    Some(if cut { text.split_once('\n').map(|(_, rest)| rest.to_string()).unwrap_or_default() } else { text.into_owned() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_three_conversations_over_every_project_are_taken() {
        let dir = tempfile::tempdir().unwrap();
        for (project, name) in [("a", "1.jsonl"), ("a", "2.jsonl"), ("b", "3.jsonl"), ("b", "4.jsonl"), ("b", "note.txt")] {
            std::fs::create_dir_all(dir.path().join(project)).unwrap();
            std::fs::write(dir.path().join(project).join(name), name).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let parts = newest(Some(dir.path()));
        let names: Vec<&str> = parts.iter().map(|part| part.name.as_str()).collect();
        assert_eq!(names, ["conversations/1-4.jsonl", "conversations/2-3.jsonl", "conversations/3-2.jsonl"]);
    }

    #[test]
    fn a_long_conversation_keeps_its_end_from_a_whole_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        let line = format!("{}\n", "x".repeat(999));
        std::fs::write(&path, line.repeat(3000)).unwrap();
        let text = tail(&path).unwrap();
        assert!(text.len() as u64 <= TAIL_BYTES && text.lines().all(|row| row.len() == 999));
    }

    #[test]
    fn with_no_folder_there_is_nothing() {
        assert!(newest(None).is_empty());
    }
}
