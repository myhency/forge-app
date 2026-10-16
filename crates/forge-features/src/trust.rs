//! What happens when a folder is added to a session: judged without any UI, so the rule is tested
//! apart from the composer's signals.

use std::path::{Path, PathBuf};

use forge_domain::is_trusted;

/// What the composer does with a folder the person picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderAdd {
    /// It is a chip already: nothing to do.
    AlreadyAdded,
    /// On the trusted list (or below it): add the chip at once.
    Trusted,
    /// Not trusted: show the question first.
    NeedsAsking,
}

/// `picked` is what the dialog returned (the chip shows it); `resolved` is the same folder through
/// `resolved()`, which is what the trusted list is compared with. A chip counts as the same folder
/// when it is the same text or resolves to the same place.
pub fn classify_added_folder(trusted: &[PathBuf], added: &[PathBuf], picked: &Path, resolved_picked: &Path) -> FolderAdd {
    if added.iter().any(|existing| existing == picked || resolved(existing) == resolved_picked) {
        FolderAdd::AlreadyAdded
    } else if is_trusted(&with_resolved_entries(trusted), resolved_picked) {
        FolderAdd::Trusted
    } else {
        FolderAdd::NeedsAsking
    }
}

// The list as written plus each entry through `resolved()`, so an entry written through a symlink
// (a hand edit, an older config) still matches the folder the picker resolved.
fn with_resolved_entries(trusted: &[PathBuf]) -> Vec<PathBuf> {
    trusted.iter().cloned().chain(trusted.iter().map(|entry| resolved(entry))).collect()
}

/// The folder through symlinks (`/tmp` is `/private/tmp` on macOS); the folder as given when it
/// cannot be read. On Windows the `\\?\` prefix `canonicalize` adds is taken off, so the list and the
/// dialog show the path the way the person knows it.
pub fn resolved(path: &Path) -> PathBuf {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if cfg!(windows) {
        forge_domain::without_extended_prefix(&path)
    } else {
        path
    }
}

/// Adds a folder chip, once. A new chip has not been announced to the model yet.
pub fn push_folder(folders: &mut Vec<(PathBuf, bool)>, path: PathBuf) {
    if !folders.iter().any(|(existing, _)| *existing == path) {
        folders.push((path, false));
    }
}

/// What a pick does to the chips and the card. A pick that needs asking opens the card (replacing an
/// open one); any other pick settles itself, and an open card for another folder closes with it.
pub fn apply_pick(folders: &mut Vec<(PathBuf, bool)>, pending: &mut Option<PathBuf>, add: FolderAdd, picked: PathBuf) {
    match add {
        FolderAdd::AlreadyAdded => *pending = None,
        FolderAdd::Trusted => {
            push_folder(folders, picked);
            *pending = None;
        }
        FolderAdd::NeedsAsking => *pending = Some(picked),
    }
}

/// The three buttons of the card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustChoice {
    TrustAndAdd,
    AddOnce,
    Cancel,
}

/// Closes the card. The chip is added before anything is saved, so a failed save loses nothing.
/// Returns the folder to put on the trusted list (`TrustAndAdd` only); `None` with no card open.
pub fn answer_card(folders: &mut Vec<(PathBuf, bool)>, pending: &mut Option<PathBuf>, choice: TrustChoice) -> Option<PathBuf> {
    let path = pending.take()?;
    match choice {
        TrustChoice::Cancel => None,
        TrustChoice::AddOnce => {
            push_folder(folders, path);
            None
        }
        TrustChoice::TrustAndAdd => {
            push_folder(folders, path.clone());
            Some(path)
        }
    }
}

/// A list without repeats, in its order (a hand-edited config can repeat a folder).
pub fn dedup_paths(list: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen: Vec<PathBuf> = Vec::new();
    for path in list {
        if !seen.contains(path) {
            seen.push(path.clone());
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chips(paths: &[&str]) -> Vec<(PathBuf, bool)> {
        paths.iter().map(|path| (PathBuf::from(path), false)).collect()
    }

    #[test]
    fn a_pick_that_needs_asking_opens_the_card_and_a_second_one_replaces_it() {
        let mut folders = Vec::new();
        let mut pending = None;
        apply_pick(&mut folders, &mut pending, FolderAdd::NeedsAsking, "/a".into());
        assert_eq!(pending, Some(PathBuf::from("/a")));
        apply_pick(&mut folders, &mut pending, FolderAdd::NeedsAsking, "/b".into());
        assert_eq!(pending, Some(PathBuf::from("/b")));
        assert!(folders.is_empty(), "nothing is added before the answer");
    }

    #[test]
    fn a_trusted_or_repeated_pick_closes_an_open_card_for_another_folder() {
        let mut folders = chips(&["/known"]);
        let mut pending = Some(PathBuf::from("/a"));
        apply_pick(&mut folders, &mut pending, FolderAdd::Trusted, "/t".into());
        assert_eq!(pending, None);
        assert_eq!(folders, chips(&["/known", "/t"]));
        pending = Some(PathBuf::from("/a"));
        apply_pick(&mut folders, &mut pending, FolderAdd::AlreadyAdded, "/known".into());
        assert_eq!(pending, None);
        assert_eq!(folders, chips(&["/known", "/t"]));
    }

    #[test]
    fn trust_and_add_adds_the_chip_and_hands_back_the_folder_to_save() {
        let mut folders = Vec::new();
        let mut pending = Some(PathBuf::from("/a"));
        let to_save = answer_card(&mut folders, &mut pending, TrustChoice::TrustAndAdd);
        assert_eq!(to_save, Some(PathBuf::from("/a")));
        assert_eq!(folders, chips(&["/a"]), "the chip does not wait for the save, so a failed save loses nothing");
        assert_eq!(pending, None);
    }

    #[test]
    fn add_this_time_adds_the_chip_and_saves_nothing() {
        let mut folders = Vec::new();
        let mut pending = Some(PathBuf::from("/a"));
        assert_eq!(answer_card(&mut folders, &mut pending, TrustChoice::AddOnce), None);
        assert_eq!(folders, chips(&["/a"]));
        assert_eq!(pending, None);
    }

    #[test]
    fn cancel_closes_the_card_and_adds_nothing() {
        let mut folders = Vec::new();
        let mut pending = Some(PathBuf::from("/a"));
        assert_eq!(answer_card(&mut folders, &mut pending, TrustChoice::Cancel), None);
        assert!(folders.is_empty());
        assert_eq!(pending, None);
    }

    #[test]
    fn an_answer_with_no_card_open_changes_nothing() {
        let mut folders = chips(&["/x"]);
        let mut pending = None;
        for choice in [TrustChoice::TrustAndAdd, TrustChoice::AddOnce, TrustChoice::Cancel] {
            assert_eq!(answer_card(&mut folders, &mut pending, choice), None);
        }
        assert_eq!(folders, chips(&["/x"]));
    }

    #[test]
    fn a_folder_chip_is_added_once() {
        let mut folders = vec![(PathBuf::from("/a"), true)];
        push_folder(&mut folders, PathBuf::from("/a"));
        push_folder(&mut folders, PathBuf::from("/b"));
        assert_eq!(folders, vec![(PathBuf::from("/a"), true), (PathBuf::from("/b"), false)]);
    }

    #[test]
    fn a_list_shows_each_folder_once_in_its_order() {
        assert_eq!(dedup_paths(&list(&["/a", "/b", "/a", "/c", "/b"])), list(&["/a", "/b", "/c"]));
        assert!(dedup_paths(&[]).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_already_added_under_another_spelling_is_not_added_twice() {
        let base = std::env::temp_dir().join(format!("forge-trust-chip-{}", std::process::id()));
        let real = base.join("real");
        let link = base.join("link");
        std::fs::create_dir_all(&real).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let added = vec![link.clone()];
        let verdict = classify_added_folder(&[], &added, &real, &resolved(&real));
        std::fs::remove_dir_all(&base).unwrap();
        assert_eq!(verdict, FolderAdd::AlreadyAdded);
    }

    #[cfg(unix)]
    #[test]
    fn a_listed_entry_written_through_a_symlink_still_matches_the_resolved_folder() {
        let base = std::env::temp_dir().join(format!("forge-trust-link-{}", std::process::id()));
        let real = base.join("real");
        let link = base.join("link");
        std::fs::create_dir_all(real.join("sub")).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let trusted = vec![link.clone()];
        let picked = real.join("sub");
        let verdict = classify_added_folder(&trusted, &[], &picked, &resolved(&picked));
        std::fs::remove_dir_all(&base).unwrap();
        assert_eq!(verdict, FolderAdd::Trusted);
    }

    fn list(paths: &[&str]) -> Vec<PathBuf> {
        paths.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn a_folder_that_is_already_a_chip_is_not_added_again() {
        let added = list(&["/work/app"]);
        assert_eq!(classify_added_folder(&[], &added, Path::new("/work/app"), Path::new("/work/app")), FolderAdd::AlreadyAdded);
    }

    #[test]
    fn a_trusted_folder_or_one_below_it_is_added_without_asking() {
        let trusted = list(&["/work"]);
        assert_eq!(classify_added_folder(&trusted, &[], Path::new("/work"), Path::new("/work")), FolderAdd::Trusted);
        assert_eq!(classify_added_folder(&trusted, &[], Path::new("/work/app"), Path::new("/work/app")), FolderAdd::Trusted);
    }

    #[test]
    fn any_other_folder_asks() {
        let trusted = list(&["/work/app"]);
        assert_eq!(classify_added_folder(&trusted, &[], Path::new("/work/app-old"), Path::new("/work/app-old")), FolderAdd::NeedsAsking);
        assert_eq!(classify_added_folder(&[], &[], Path::new("/tmp"), Path::new("/tmp")), FolderAdd::NeedsAsking);
    }

    #[test]
    fn the_trust_is_judged_on_the_resolved_path_but_the_chip_on_the_picked_one() {
        let trusted = list(&["/private/tmp/x"]);
        assert_eq!(classify_added_folder(&trusted, &[], Path::new("/tmp/x"), Path::new("/private/tmp/x")), FolderAdd::Trusted);
        let added = list(&["/tmp/x"]);
        assert_eq!(classify_added_folder(&trusted, &added, Path::new("/tmp/x"), Path::new("/private/tmp/x")), FolderAdd::AlreadyAdded);
    }

    #[test]
    fn a_path_that_does_not_exist_resolves_to_itself() {
        let gone = Path::new("/definitely/not/here/forge-43");
        assert_eq!(resolved(gone), gone);
    }
}
