//! Scratch folders: empty folders Forge makes for sessions started without a folder.

use std::path::{Component, Path, PathBuf};

/// Folder under Forge's data directory that holds the scratch folders.
pub const SCRATCH_DIR: &str = "scratch-workspaces";

/// `scratch-YYYY-MM-DD-xxxxxx`, where `xxxxxx` is the low 24 bits of `seed` in hex.
pub fn scratch_folder_name(year: i32, month: u8, day: u8, seed: u64) -> String {
    format!("scratch-{year:04}-{month:02}-{day:02}-{:06x}", seed & 0xFF_FFFF)
}

/// True only for a direct child of `root` whose name is `scratch-` plus a non-empty suffix.
pub fn is_scratch(root: &Path, path: &Path) -> bool {
    if path.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
        return false;
    }
    if path.parent().map(|p| p.components().eq(root.components())) != Some(true) {
        return false;
    }
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.strip_prefix("scratch-").is_some_and(|rest| !rest.is_empty()))
}

/// The scratch root next to the config file.
pub fn scratch_root(config_file: &Path) -> Option<PathBuf> {
    Some(config_file.parent()?.join(SCRATCH_DIR))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_is_padded_and_uses_low_24_bits() {
        assert_eq!(scratch_folder_name(2026, 10, 20, 0xab12cd), "scratch-2026-10-20-ab12cd");
        assert_eq!(scratch_folder_name(2026, 1, 5, 0x1), "scratch-2026-01-05-000001");
        assert_eq!(scratch_folder_name(2026, 3, 9, 0xFFFF_FFFF_ffab_12cd), "scratch-2026-03-09-ab12cd");
    }

    #[test]
    fn is_scratch_cases() {
        let root = Path::new("/c/Forge/scratch-workspaces");
        assert!(is_scratch(root, &root.join("scratch-2026-10-20-ab12cd")));
        assert!(!is_scratch(root, root));
        assert!(!is_scratch(root, &root.join("scratch-x").join("deeper")));
        assert!(!is_scratch(root, &root.join("other")));
        assert!(!is_scratch(root, Path::new("/elsewhere/scratch-2026-10-20-ab12cd")));
        assert!(!is_scratch(root, &root.join("..").join("scratch-a")));
    }

    #[test]
    fn is_scratch_edge_cases() {
        let root = Path::new("/c/Forge/scratch-workspaces");
        assert!(!is_scratch(root, &root.join("scratch-")));
        assert!(!is_scratch(root, &root.join("scratch-a").join("..")));
        assert!(!is_scratch(Path::new("rel/root"), Path::new("./scratch-a")));
        assert!(!is_scratch(Path::new("./rel/root"), Path::new("./rel/root/scratch-a")));
        assert!(is_scratch(Path::new("/c/Forge/scratch-workspaces/"), &root.join("scratch-a")));
    }

    #[test]
    fn scratch_root_is_next_to_config() {
        assert_eq!(
            scratch_root(Path::new("/c/Forge/config.json")),
            Some(PathBuf::from("/c/Forge/scratch-workspaces"))
        );
        assert_eq!(scratch_root(Path::new("/")), None);
    }
}
