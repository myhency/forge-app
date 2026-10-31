use std::path::PathBuf;
use forge_shared::tr;

/// What to do with a folder the person picked in the dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderChoice {
    /// Already offered in the dropdown: select that project.
    Known(PathBuf),
    /// A folder with no Claude Code history yet: offer it and select it.
    Added(PathBuf),
    /// Not usable; the message says why.
    Rejected(String),
}

/// Decides what picking `picked` means. `known` are the folders the dropdown already offers;
/// `is_dir` is whether `picked` is an existing directory (the caller asks the file system, so this
/// stays pure). Trailing separators do not make a path different.
pub fn choose_folder(known: &[PathBuf], picked: PathBuf, is_dir: bool) -> FolderChoice {
    if !is_dir {
        return FolderChoice::Rejected(tr!("{path} is not a folder that exists.", path = picked.display()));
    }
    // `components()` ignores a trailing separator, so `/work/b/` equals `/work/b`.
    let normalized: PathBuf = picked.components().collect();
    match known.iter().find(|path| path.components().eq(normalized.components())) {
        Some(existing) => FolderChoice::Known(existing.clone()),
        None => FolderChoice::Added(normalized),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> Vec<PathBuf> {
        vec![PathBuf::from("/work/a"), PathBuf::from("/work/b")]
    }

    #[test]
    fn a_folder_already_in_the_list_selects_that_project() {
        assert_eq!(
            choose_folder(&known(), PathBuf::from("/work/b"), true),
            FolderChoice::Known(PathBuf::from("/work/b"))
        );
    }

    #[test]
    fn a_trailing_slash_does_not_make_a_known_folder_new() {
        assert_eq!(
            choose_folder(&known(), PathBuf::from("/work/b/"), true),
            FolderChoice::Known(PathBuf::from("/work/b"))
        );
    }

    #[test]
    fn a_new_existing_folder_is_added() {
        assert_eq!(
            choose_folder(&known(), PathBuf::from("/work/new"), true),
            FolderChoice::Added(PathBuf::from("/work/new"))
        );
    }

    #[test]
    fn a_path_that_is_not_a_directory_is_rejected_even_if_it_is_listed() {
        for path in ["/work/a", "/work/missing"] {
            let choice = choose_folder(&known(), PathBuf::from(path), false);
            assert!(matches!(&choice, FolderChoice::Rejected(m) if m.contains(path)), "{choice:?}");
        }
    }
}
