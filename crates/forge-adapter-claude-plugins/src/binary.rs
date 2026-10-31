use std::path::{Path, PathBuf};

/// The folder name the SDK uses for a platform's Claude package, or `None` for a platform it has none for.
fn sdk_platform(os: &str, arch: &str, musl: bool) -> Option<String> {
    let os = match os {
        "macos" => "darwin",
        "windows" => "win32",
        "linux" => "linux",
        _ => return None,
    };
    let arch = match arch {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        _ => return None,
    };
    // Alpine and the like: the SDK has a separate package for a Linux built on musl.
    let libc = if musl && os == "linux" { "-musl" } else { "" };
    Some(format!("{os}-{arch}{libc}"))
}

/// Where the Claude program that the SDK ships is, from the sidecar script
/// (`<...>/sidecar/dist/index.js`): `<...>/sidecar/node_modules/@anthropic-ai/claude-agent-sdk-<platform>/claude`.
/// A development build and a packaged one keep the same layout (`scripts/package/stage-runtime.mjs`).
fn claude_binary(sidecar_script: &Path, os: &str, arch: &str, musl: bool) -> Option<PathBuf> {
    let sidecar = sidecar_script.parent()?.parent()?;
    let file = if os == "windows" { "claude.exe" } else { "claude" };
    Some(
        sidecar
            .join("node_modules")
            .join("@anthropic-ai")
            .join(format!("claude-agent-sdk-{}", sdk_platform(os, arch, musl)?))
            .join(file),
    )
}

/// `claude_binary` for the machine Forge is running on.
pub fn claude_binary_for(sidecar_script: &Path) -> Option<PathBuf> {
    claude_binary(sidecar_script, std::env::consts::OS, std::env::consts::ARCH, cfg!(target_env = "musl"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sdk_names_its_platform_packages_darwin_win32_and_arm64_x64() {
        assert_eq!(sdk_platform("macos", "aarch64", false).as_deref(), Some("darwin-arm64"));
        assert_eq!(sdk_platform("macos", "x86_64", false).as_deref(), Some("darwin-x64"));
        assert_eq!(sdk_platform("windows", "x86_64", false).as_deref(), Some("win32-x64"));
        assert_eq!(sdk_platform("windows", "aarch64", false).as_deref(), Some("win32-arm64"));
        assert_eq!(sdk_platform("linux", "x86_64", false).as_deref(), Some("linux-x64"));
        assert_eq!(sdk_platform("freebsd", "x86_64", false), None);
        assert_eq!(sdk_platform("macos", "riscv64", false), None);
    }

    #[test]
    fn a_musl_linux_uses_the_musl_package() {
        assert_eq!(sdk_platform("linux", "x86_64", true).as_deref(), Some("linux-x64-musl"));
        assert_eq!(sdk_platform("linux", "aarch64", true).as_deref(), Some("linux-arm64-musl"));
        assert_eq!(sdk_platform("macos", "aarch64", true).as_deref(), Some("darwin-arm64"), "musl only means something on Linux");
    }

    #[test]
    fn the_binary_sits_in_node_modules_beside_the_sidecars_dist_folder() {
        let path = claude_binary("/runtime/sidecar/dist/index.js".as_ref(), "macos", "aarch64", false).unwrap();
        assert_eq!(path, PathBuf::from("/runtime/sidecar/node_modules/@anthropic-ai/claude-agent-sdk-darwin-arm64/claude"));
    }

    #[test]
    fn on_windows_the_file_is_claude_exe() {
        let path = claude_binary("C:/runtime/sidecar/dist/index.js".as_ref(), "windows", "x86_64", false).unwrap();
        assert!(path.ends_with("claude-agent-sdk-win32-x64/claude.exe"), "{path:?}");
    }

    #[test]
    fn a_script_path_with_no_parent_folders_gives_none() {
        assert_eq!(claude_binary("index.js".as_ref(), "macos", "aarch64", false), None);
    }
}
