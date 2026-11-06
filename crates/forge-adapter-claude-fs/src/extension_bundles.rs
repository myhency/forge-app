//! Unpacks desktop extensions (`.mcpb`, `.dxt`) into Forge's extensions folder.

use std::io::Read;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use forge_ports::ExtensionBundlePort;

// An extension carries its own node_modules: larger than a skill, still bounded.
const ENTRY_LIMIT: usize = 60_000;
const SIZE_LIMIT: u64 = 500 * 1024 * 1024;

pub struct ExtensionBundles {
    root: PathBuf,
    node: Option<PathBuf>,
}

impl ExtensionBundles {
    /// `root` is where extensions live (`<Forge config>/extensions`); `node` is Forge's own Node.
    pub fn new(root: PathBuf, node: Option<PathBuf>) -> Self {
        Self { root, node }
    }
}

#[async_trait]
impl ExtensionBundlePort for ExtensionBundles {
    async fn unpack(&self, archive: &Path) -> Result<(PathBuf, String), String> {
        let (root, archive) = (self.root.clone(), archive.to_path_buf());
        tokio::task::spawn_blocking(move || install(&root, &archive)).await.map_err(|error| error.to_string())?
    }

    fn node_program(&self) -> Option<PathBuf> {
        self.node.clone().filter(|node| node.is_file())
    }
}

fn install(root: &Path, archive: &Path) -> Result<(PathBuf, String), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    let temp = root.join(format!(".unpack-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp).map_err(|error| error.to_string())?;
    let result = unpack(archive, &temp).and_then(|()| {
        let manifest = std::fs::read_to_string(temp.join("manifest.json")).map_err(|_| "The file has no manifest.json at its top: it is not a desktop extension.".to_string())?;
        let parsed = forge_domain::read_manifest(&manifest)?;
        let dest = root.join(forge_domain::bundle_server_name(&parsed.name));
        if dest.exists() {
            std::fs::remove_dir_all(&dest).map_err(|error| error.to_string())?;
        }
        std::fs::rename(&temp, &dest).map_err(|error| error.to_string())?;
        Ok((dest, manifest))
    });
    if temp.exists() {
        let _ = std::fs::remove_dir_all(&temp);
    }
    result
}

// Refuses anything that could write outside `into`, links, and archives past the limits.
fn unpack(archive: &Path, into: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|error| error.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| "That file is not a desktop extension (.mcpb or .dxt).".to_string())?;
    if zip.len() > ENTRY_LIMIT {
        return Err("The extension has too many files.".to_string());
    }
    let mut total: u64 = 0;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|error| error.to_string())?;
        let Some(relative) = entry.enclosed_name() else {
            return Err("The extension has a file outside its own folder.".to_string());
        };
        if entry.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) {
            return Err("The extension has a link in it.".to_string());
        }
        let target = into.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&target).map_err(|error| error.to_string())?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut out = std::fs::File::create(&target).map_err(|error| error.to_string())?;
        let written = std::io::copy(&mut (&mut entry).take(SIZE_LIMIT - total + 1), &mut out).map_err(|error| error.to_string())?;
        total += written;
        if total > SIZE_LIMIT {
            return Err("The extension is too large.".to_string());
        }
        // A bundled program keeps its permission to run.
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(mode & 0o755));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn bundle(dir: &Path, files: &[(&str, &str)]) -> PathBuf {
        let path = dir.join("ext.mcpb");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (name, text) in files {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    #[test]
    fn an_extension_is_unpacked_under_its_name_and_replaces_an_earlier_copy() {
        let temp = tempfile::tempdir().unwrap();
        let manifest = r#"{"name":"Hello Tool","server":{"mcp_config":{"command":"node","args":["${__dirname}/index.js"]}}}"#;
        let archive = bundle(temp.path(), &[("manifest.json", manifest), ("index.js", "console.log(1)")]);
        let root = temp.path().join("extensions");
        let (dir, text) = install(&root, &archive).unwrap();
        assert_eq!(dir, root.join("Hello-Tool"));
        assert_eq!(text, manifest);
        assert!(dir.join("index.js").is_file());
        std::fs::write(dir.join("old.txt"), "x").unwrap();
        install(&root, &archive).unwrap();
        assert!(!dir.join("old.txt").exists(), "a new install replaces the old folder");
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1, "no unpack folder is left behind");
    }

    #[test]
    fn a_zip_without_a_manifest_is_refused_and_leaves_nothing() {
        let temp = tempfile::tempdir().unwrap();
        let archive = bundle(temp.path(), &[("index.js", "x")]);
        let root = temp.path().join("extensions");
        assert!(install(&root, &archive).unwrap_err().contains("manifest.json"));
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    }
}
