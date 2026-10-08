//! Where the Node program and the sidecar script are. A packaged Forge carries both in a `runtime`
//! folder beside it, so nothing has to be installed; a development build uses the Node on the PATH
//! and the sidecar built in this repository. `FORGE_NODE` and `FORGE_SIDECAR_SCRIPT` override either.

use std::path::{Path, PathBuf};

pub struct Runtime {
    pub node: String,
    pub script: String,
}

const NODE_FILE: &str = if cfg!(windows) { "node.exe" } else { "node" };

// The folders a package may keep `runtime` in, from the executable's folder: beside it (Windows,
// Linux) and in `Contents/Resources` of a macOS app bundle.
fn runtime_dirs(exe_dir: &Path) -> [PathBuf; 2] {
    [exe_dir.join("runtime"), exe_dir.join("../Resources/runtime")]
}

/// `exists` answers whether a path is a file; tests pass their own.
pub fn locate(
    exe_dir: Option<&Path>,
    node_override: Option<String>,
    script_override: Option<String>,
    development_script: &str,
    exists: impl Fn(&Path) -> bool,
) -> Runtime {
    let bundled = exe_dir.into_iter().flat_map(runtime_dirs).find(|dir| exists(&dir.join(NODE_FILE)) && exists(&dir.join("sidecar/dist/index.js")));
    let node = node_override.filter(|node| !node.is_empty()).unwrap_or_else(|| match &bundled {
        Some(dir) => dir.join(NODE_FILE).to_string_lossy().into_owned(),
        None => "node".to_string(),
    });
    let script = script_override.filter(|script| !script.is_empty()).unwrap_or_else(|| match &bundled {
        Some(dir) => dir.join("sidecar/dist/index.js").to_string_lossy().into_owned(),
        None => development_script.to_string(),
    });
    Runtime { node, script }
}

pub fn locate_here(development_script: &str) -> Runtime {
    let exe = std::env::current_exe().ok();
    locate(
        exe.as_deref().and_then(Path::parent),
        std::env::var("FORGE_NODE").ok(),
        std::env::var("FORGE_SIDECAR_SCRIPT").ok(),
        development_script,
        Path::is_file,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEV: &str = "/repo/sidecar/dist/index.js";

    // A file system that has exactly these files (written with `/`, whatever the platform).
    fn with(files: Vec<String>) -> impl Fn(&Path) -> bool {
        move |path: &Path| files.contains(&path.to_string_lossy().replace('\\', "/"))
    }

    fn runtime_in(dir: &str) -> Vec<String> {
        vec![format!("{dir}/{NODE_FILE}"), format!("{dir}/sidecar/dist/index.js")]
    }

    fn slashes(text: &str) -> String {
        text.replace('\\', "/")
    }

    #[test]
    fn a_development_build_uses_the_node_on_the_path_and_the_built_sidecar() {
        let runtime = locate(Some(Path::new("/repo/target/debug")), None, None, DEV, with(vec![]));
        assert_eq!((runtime.node.as_str(), runtime.script.as_str()), ("node", DEV));
    }

    #[test]
    fn a_runtime_folder_beside_the_program_is_used() {
        let runtime = locate(Some(Path::new("/app")), None, None, DEV, with(runtime_in("/app/runtime")));
        assert_eq!(slashes(&runtime.node), format!("/app/runtime/{NODE_FILE}"));
        assert_eq!(slashes(&runtime.script), "/app/runtime/sidecar/dist/index.js");
    }

    #[test]
    fn a_mac_app_bundle_keeps_it_in_resources() {
        let files = runtime_in("/Forge.app/Contents/MacOS/../Resources/runtime");
        let runtime = locate(Some(Path::new("/Forge.app/Contents/MacOS")), None, None, DEV, with(files));
        assert!(slashes(&runtime.script).ends_with("Resources/runtime/sidecar/dist/index.js"), "{}", runtime.script);
    }

    #[test]
    fn a_runtime_folder_missing_its_script_is_not_used() {
        let runtime = locate(Some(Path::new("/app")), None, None, DEV, with(vec![format!("/app/runtime/{NODE_FILE}")]));
        assert_eq!((runtime.node.as_str(), runtime.script.as_str()), ("node", DEV));
    }

    #[test]
    fn the_environment_overrides_a_bundled_runtime_and_an_empty_value_is_ignored() {
        let runtime = locate(Some(Path::new("/app")), Some("/opt/node".into()), Some("/x/index.js".into()), DEV, with(runtime_in("/app/runtime")));
        assert_eq!((runtime.node.as_str(), runtime.script.as_str()), ("/opt/node", "/x/index.js"));
        let empty = locate(None, Some(String::new()), Some(String::new()), DEV, with(vec![]));
        assert_eq!((empty.node.as_str(), empty.script.as_str()), ("node", DEV));
    }
}
