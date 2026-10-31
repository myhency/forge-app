//! Runs the real `claude` that ships with the SDK against a temporary config folder and a local
//! marketplace. Skipped when the program is not installed (`npm ci` in `sidecar/` installs it).

use std::path::PathBuf;

use forge_adapter_claude_plugins::{claude_binary_for, ClaudePluginCli};
use forge_ports::PluginManagerPort;

fn binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("FORGE_CLAUDE_BIN") {
        return Some(PathBuf::from(path));
    }
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sidecar/dist/index.js");
    claude_binary_for(&script).filter(|path| path.is_file())
}

fn write(path: PathBuf, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[tokio::test]
async fn a_local_marketplace_can_be_added_installed_from_uninstalled_and_removed() {
    let Some(binary) = binary() else {
        eprintln!("skipped: the bundled claude is not installed");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let market = tmp.path().join("market");
    write(
        market.join(".claude-plugin/marketplace.json"),
        r#"{"name":"test-market","owner":{"name":"t"},"plugins":[
            {"name":"hello","source":"./plugins/hello","description":"first"},
            {"name":"second","source":"./plugins/second","description":"second"}]}"#,
    );
    write(market.join("plugins/hello/.claude-plugin/plugin.json"), r#"{"name":"hello","version":"1.0.0","description":"first"}"#);
    write(market.join("plugins/second/.claude-plugin/plugin.json"), r#"{"name":"second","version":"0.2.0","description":"second"}"#);

    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let cli = ClaudePluginCli::new(binary, home.clone(), Some(home));

    cli.add_marketplace(market.to_str().unwrap()).await.unwrap();
    let snapshot = cli.snapshot().await.unwrap();
    assert_eq!(snapshot.marketplaces.len(), 1);
    assert_eq!(snapshot.marketplaces[0].name, "test-market");
    let ids: Vec<&str> = snapshot.plugins.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["hello@test-market", "second@test-market"]);
    assert!(snapshot.plugins.iter().all(|p| !p.installed));

    cli.install("hello@test-market").await.unwrap();
    let snapshot = cli.snapshot().await.unwrap();
    let hello = snapshot.plugins.iter().find(|p| p.id == "hello@test-market").unwrap();
    assert!(hello.installed);
    assert_eq!(hello.description, "first", "the description is read from the installed manifest");
    assert!(!snapshot.plugins.iter().find(|p| p.id == "second@test-market").unwrap().installed);

    cli.uninstall("hello@test-market").await.unwrap();
    assert!(!cli.snapshot().await.unwrap().plugins.iter().any(|p| p.installed));

    let error = cli.install("nope@test-market").await.unwrap_err();
    assert!(error.to_string().contains("not found"), "{error}");

    cli.remove_marketplace("test-market").await.unwrap();
    assert!(cli.snapshot().await.unwrap().marketplaces.is_empty());
}
