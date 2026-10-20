use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use forge_domain::{is_scratch, scratch_folder_name, without_extended_prefix};
use forge_ports::{ScratchError, ScratchPort};

/// How many seeds `create` tries before it gives up.
const CREATE_ATTEMPTS: u64 = 5;

/// A resolved path as Forge hands it out and compares it: Windows `canonicalize` gives a `\\?\` verbatim
/// path, which Claude Code does not write in its transcripts, so the prefix comes off there.
fn display_path(path: PathBuf) -> PathBuf {
    if cfg!(windows) {
        without_extended_prefix(&path)
    } else {
        path
    }
}

/// What releasing a scratch folder did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScratchRelease {
    /// The empty folder is gone.
    Removed,
    /// The folder holds something (or is already gone) and is left alone.
    Kept,
    /// Not a scratch folder Forge made; it is never touched.
    NotScratch,
}

/// Makes and releases the empty folders for sessions started without a project folder.
pub struct ScratchUseCase {
    root: Option<PathBuf>,
    /// The root with symlinks resolved, once it exists: Claude Code records the real path as a
    /// session's folder, so that is the one sessions must start in and be matched against.
    canonical: OnceLock<PathBuf>,
    port: Arc<dyn ScratchPort>,
}

impl ScratchUseCase {
    pub fn new(root: Option<PathBuf>, port: Arc<dyn ScratchPort>) -> Self {
        let uc = Self { root, canonical: OnceLock::new(), port };
        uc.resolve();
        uc
    }

    /// The canonical root when the root exists, otherwise the configured one.
    fn resolve(&self) -> Option<&Path> {
        let root = self.root.as_deref()?;
        if let Some(real) = self.canonical.get() {
            return Some(real);
        }
        match root.canonicalize() {
            Ok(real) => Some(self.canonical.get_or_init(|| display_path(real))),
            Err(_) => Some(root),
        }
    }

    pub fn available(&self) -> bool {
        self.root.is_some()
    }

    pub fn root(&self) -> Option<&Path> {
        self.resolve()
    }

    /// Makes sure the root itself exists (a folder to probe slash commands in) and returns it.
    pub async fn ensure_root(&self) -> Result<PathBuf, ScratchError> {
        let root = self.root.as_ref().ok_or_else(|| ScratchError("No place to make scratch folders".into()))?;
        // `false` only says it was already there.
        self.port.create_new_dir(root).await?;
        Ok(self.resolve().unwrap_or(root).to_path_buf())
    }

    /// Makes a new empty folder `<root>/scratch-YYYY-MM-DD-xxxxxx`. A name that is taken moves on to
    /// the next seed, so two sessions never share a folder.
    pub async fn create(&self, year: i32, month: u8, day: u8, seed: u64) -> Result<PathBuf, ScratchError> {
        let root = self.resolve().ok_or_else(|| ScratchError("No place to make scratch folders".into()))?.to_path_buf();
        for attempt in 0..CREATE_ATTEMPTS {
            let path = root.join(scratch_folder_name(year, month, day, seed.wrapping_add(attempt)));
            if self.port.create_new_dir(&path).await? {
                // The root may only have come into being just now; the made folder resolves to the real path.
                return Ok(match path.canonicalize() {
                    Ok(real) if real.file_name() == path.file_name() => display_path(real),
                    _ => path,
                });
            }
        }
        Err(ScratchError("Could not find a free scratch folder name".into()))
    }

    /// Removes `path` when it is an empty scratch folder Forge made. Deletion gate: only a direct child
    /// of the root whose name passes `is_scratch`, and that is still that same direct child after
    /// symlinks resolve; removal never takes contents with it.
    pub async fn release(&self, path: &Path) -> Result<ScratchRelease, ScratchError> {
        let Some(root) = self.resolve() else {
            return Ok(ScratchRelease::NotScratch);
        };
        if !is_scratch(root, path) {
            return Ok(ScratchRelease::NotScratch);
        }
        let canonical_root = display_path(root.canonicalize().unwrap_or_else(|_| root.to_path_buf()));
        match path.canonicalize() {
            Ok(real) => {
                let real = display_path(real);
                // A symlink (or a link in the way) resolves elsewhere or under another name.
                if real.parent() != Some(canonical_root.as_path()) || real.file_name() != path.file_name() {
                    return Ok(ScratchRelease::NotScratch);
                }
            }
            Err(_) => {
                // Nothing there to remove; a dangling link is something, but not ours.
                return Ok(if path.symlink_metadata().is_ok() { ScratchRelease::NotScratch } else { ScratchRelease::Kept });
            }
        }
        if !self.port.is_empty_dir(path).await? {
            return Ok(ScratchRelease::Kept);
        }
        self.port.remove_empty_dir(path).await?;
        Ok(ScratchRelease::Removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::fs;

    #[test]
    fn the_verbatim_prefix_is_taken_off_a_resolved_path() {
        assert_eq!(without_extended_prefix(Path::new(r"\\?\C:\x\scratch-workspaces")), PathBuf::from(r"C:\x\scratch-workspaces"));
        assert_eq!(display_path(PathBuf::from("/a/b")), PathBuf::from("/a/b"));
    }

    /// A std-fs port, so the checks run against a real folder tree.
    struct FsPort;

    #[async_trait]
    impl ScratchPort for FsPort {
        async fn create_new_dir(&self, path: &Path) -> Result<bool, ScratchError> {
            fs::create_dir_all(path.parent().unwrap()).map_err(|e| ScratchError(e.to_string()))?;
            match fs::create_dir(path) {
                Ok(()) => Ok(true),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
                Err(e) => Err(ScratchError(e.to_string())),
            }
        }
        async fn is_empty_dir(&self, path: &Path) -> Result<bool, ScratchError> {
            Ok(fs::read_dir(path).map(|mut d| d.next().is_none()).unwrap_or(false))
        }
        async fn remove_empty_dir(&self, path: &Path) -> Result<(), ScratchError> {
            fs::remove_dir(path).map_err(|e| ScratchError(e.to_string()))
        }
    }

    /// `<temp>/<base>/scratch-workspaces` as root; `<temp>/<base>` is the base.
    fn setup(name: &str) -> (PathBuf, PathBuf, ScratchUseCase) {
        let base = std::env::temp_dir().canonicalize().unwrap().join(format!("forge-scratchuc-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let root = base.join("scratch-workspaces");
        fs::create_dir_all(&root).unwrap();
        let uc = ScratchUseCase::new(Some(root.clone()), Arc::new(FsPort));
        (base, root, uc)
    }

    #[tokio::test]
    async fn create_makes_a_scratch_folder_under_the_root() {
        let (base, root, uc) = setup("create");
        assert!(uc.available());
        assert_eq!(uc.root(), Some(root.as_path()));
        let made = uc.create(2026, 10, 20, 0xab12cd).await.unwrap();
        assert_eq!(made, root.join("scratch-2026-10-20-ab12cd"));
        assert!(made.is_dir());
        assert!(is_scratch(&root, &made));
        fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test]
    async fn ensure_root_makes_the_root_and_a_second_call_is_fine() {
        let base = std::env::temp_dir().canonicalize().unwrap().join(format!("forge-scratchuc-ensure-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let root = base.join("scratch-workspaces");
        let uc = ScratchUseCase::new(Some(root.clone()), Arc::new(FsPort));
        assert!(!root.exists());
        assert_eq!(uc.ensure_root().await.unwrap(), root);
        assert!(root.is_dir());
        assert_eq!(uc.ensure_root().await.unwrap(), root);
        let none = ScratchUseCase::new(None, Arc::new(FsPort));
        assert!(none.ensure_root().await.is_err());
        fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test]
    async fn a_second_create_with_the_same_seed_gets_another_folder() {
        let (base, _root, uc) = setup("twice");
        let a = uc.create(2026, 10, 20, 7).await.unwrap();
        fs::write(a.join("f"), "x").unwrap();
        let b = uc.create(2026, 10, 20, 7).await.unwrap();
        let c = uc.create(2026, 10, 20, 7).await.unwrap();
        assert!(a != b && b != c && a != c);
        assert!(a.join("f").is_file());
        fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test]
    async fn release_removes_an_empty_child() {
        let (base, _root, uc) = setup("empty");
        let made = uc.create(2026, 10, 20, 1).await.unwrap();
        assert_eq!(uc.release(&made).await.unwrap(), ScratchRelease::Removed);
        assert!(!made.exists());
        fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test]
    async fn release_keeps_a_child_holding_a_file() {
        let (base, _root, uc) = setup("full");
        let made = uc.create(2026, 10, 20, 1).await.unwrap();
        fs::write(made.join("notes.txt"), "x").unwrap();
        assert_eq!(uc.release(&made).await.unwrap(), ScratchRelease::Kept);
        assert!(made.join("notes.txt").is_file());
        fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test]
    async fn release_of_a_missing_child_is_kept() {
        let (base, root, uc) = setup("missing");
        assert_eq!(uc.release(&root.join("scratch-gone")).await.unwrap(), ScratchRelease::Kept);
        fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test]
    async fn release_never_touches_anything_that_is_not_a_scratch_child() {
        let (base, root, uc) = setup("outside");
        let sibling = base.join("scratch-sibling");
        let other = base.join("other");
        let plain = root.join("plain");
        let nested = root.join("scratch-a").join("scratch-b");
        for d in [&sibling, &other, &plain, &nested] {
            fs::create_dir_all(d).unwrap();
        }
        let dotdot = root.join("scratch-a").join("..");
        for p in [root.clone(), root.join(".."), sibling.clone(), other.clone(), plain.clone(), nested.clone(), dotdot, PathBuf::from("/")] {
            assert_eq!(uc.release(&p).await.unwrap(), ScratchRelease::NotScratch, "{p:?}");
        }
        for d in [&root, &sibling, &other, &plain, &nested] {
            assert!(d.is_dir(), "{d:?}");
        }
        fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test]
    async fn without_a_root_nothing_works() {
        let uc = ScratchUseCase::new(None, Arc::new(FsPort));
        assert!(!uc.available());
        assert_eq!(uc.root(), None);
        assert!(uc.create(2026, 10, 20, 1).await.is_err());
        assert_eq!(uc.release(Path::new("/x/scratch-a")).await.unwrap(), ScratchRelease::NotScratch);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_symlink_child_is_not_scratch_and_its_target_survives() {
        let (base, root, uc) = setup("symlink");
        let target = base.join("target-empty");
        fs::create_dir_all(&target).unwrap();
        let link = root.join("scratch-link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert_eq!(uc.release(&link).await.unwrap(), ScratchRelease::NotScratch);
        assert!(target.is_dir());
        // A link to another child of the root is not that child either.
        let real = uc.create(2026, 10, 20, 9).await.unwrap();
        let alias = root.join("scratch-alias");
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        assert_eq!(uc.release(&alias).await.unwrap(), ScratchRelease::NotScratch);
        assert!(real.is_dir());
        // A dangling link is not removed either.
        let dangling = root.join("scratch-dangling");
        std::os::unix::fs::symlink(base.join("nowhere"), &dangling).unwrap();
        assert_eq!(uc.release(&dangling).await.unwrap(), ScratchRelease::NotScratch);
        fs::remove_dir_all(&base).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_root_reached_through_a_symlink_still_works() {
        let (base, root, _uc) = setup("rootlink");
        let via = base.join("via");
        std::os::unix::fs::symlink(&root, &via).unwrap();
        let uc = ScratchUseCase::new(Some(via.clone()), Arc::new(FsPort));
        let made = uc.create(2026, 10, 20, 3).await.unwrap();
        assert_eq!(uc.release(&made).await.unwrap(), ScratchRelease::Removed);
        fs::remove_dir_all(&base).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_root_behind_a_symlink_hands_out_the_canonical_root() {
        let (base, root, _uc) = setup("canon");
        let via = base.join("via");
        std::os::unix::fs::symlink(&root, &via).unwrap();
        let uc = ScratchUseCase::new(Some(via), Arc::new(FsPort));
        let canonical = root.canonicalize().unwrap();
        assert_eq!(uc.root(), Some(canonical.as_path()));
        let made = uc.create(2026, 10, 20, 5).await.unwrap();
        assert_eq!(made.parent(), Some(canonical.as_path()));
        assert!(is_scratch(uc.root().unwrap(), &made));
        assert_eq!(uc.release(&made).await.unwrap(), ScratchRelease::Removed);
        assert!(!made.exists());
        fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test]
    async fn create_takes_the_first_seed_the_port_reports_as_new() {
        let mut port = forge_ports::MockScratchPort::new();
        let mut seq = mockall::Sequence::new();
        port.expect_create_new_dir().times(1).in_sequence(&mut seq).returning(|_| Ok(false));
        port.expect_create_new_dir().times(1).in_sequence(&mut seq).returning(|_| Ok(true));
        let uc = ScratchUseCase::new(Some(PathBuf::from("/r/scratch-workspaces")), Arc::new(port));
        let made = uc.create(2026, 10, 20, 0x10).await.unwrap();
        assert_eq!(made, PathBuf::from("/r/scratch-workspaces/scratch-2026-10-20-000011"));
    }

    #[tokio::test]
    async fn create_gives_up_after_five_taken_names() {
        let mut port = forge_ports::MockScratchPort::new();
        port.expect_create_new_dir().times(5).returning(|_| Ok(false));
        let uc = ScratchUseCase::new(Some(PathBuf::from("/r/scratch-workspaces")), Arc::new(port));
        assert!(uc.create(2026, 10, 20, 1).await.is_err());
    }

    #[tokio::test]
    async fn release_with_a_root_that_does_not_exist_removes_nothing() {
        let base = std::env::temp_dir().join(format!("forge-scratchuc-noroot-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let root = base.join("scratch-workspaces");
        let uc = ScratchUseCase::new(Some(root.clone()), Arc::new(FsPort));
        let r = uc.release(&root.join("scratch-x")).await.unwrap();
        assert!(matches!(r, ScratchRelease::Kept | ScratchRelease::NotScratch));
        assert!(base.is_dir());
        fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test]
    async fn release_of_a_regular_file_leaves_it() {
        let (base, root, uc) = setup("file");
        let file = root.join("scratch-x");
        fs::write(&file, "x").unwrap();
        assert_ne!(uc.release(&file).await.unwrap(), ScratchRelease::Removed);
        assert!(file.is_file());
        fs::remove_dir_all(&base).unwrap();
    }
}
