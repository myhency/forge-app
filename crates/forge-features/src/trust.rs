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
/// `resolved()`, which is what the trusted list is compared with.
pub fn classify_added_folder(trusted: &[PathBuf], added: &[PathBuf], picked: &Path, resolved: &Path) -> FolderAdd {
    if added.iter().any(|existing| existing == picked) {
        FolderAdd::AlreadyAdded
    } else if is_trusted(trusted, resolved) {
        FolderAdd::Trusted
    } else {
        FolderAdd::NeedsAsking
    }
}

/// The folder through symlinks (`/tmp` is `/private/tmp` on macOS); the folder as given when it
/// cannot be read. Windows `\\?\` prefixes are dealt with by the comparison.
pub fn resolved(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

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
