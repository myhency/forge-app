use forge_adapter_sidecar::SidecarProcess;
use forge_domain::{ActiveExtensions, PermissionMode};
use forge_ports::{SidecarEvent, SidecarPort, StartSessionParams};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn fixture_path() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fake_sidecar.js").to_string()
}

fn node_binary() -> String {
    std::env::var("NODE_BINARY").unwrap_or_else(|_| "node".to_string())
}

fn default_params() -> StartSessionParams {
    StartSessionParams {
        cwd: "/tmp".to_string(),
        resume_session_id: None,
        permission_mode: PermissionMode::Default,
        model: None,
        effort: None,
        extensions: Default::default(),
        additional_directories: Vec::new(),
        allowed_tools: Vec::new(),
        env: forge_domain::EnvVars::default(),
        defaults: forge_domain::SessionDefaults::default(),
    }
}

#[tokio::test]
async fn start_session_and_send_message_round_trip() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &[]).unwrap();

    sidecar.start_session(default_params()).await.unwrap();
    let started = sidecar.next_event().await.unwrap();
    assert!(matches!(started, SidecarEvent::SessionStarted { .. }));

    sidecar.send_message("hello".to_string(), Vec::new()).await.unwrap();
    match sidecar.next_event().await.unwrap() {
        SidecarEvent::AssistantDelta { text } => assert_eq!(text, "echo:hello"),
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn attachments_and_extra_folders_reach_the_sidecar() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &[]).unwrap();
    let mut params = default_params();
    params.additional_directories = vec!["/extra".to_string()];
    sidecar.start_session(params).await.unwrap();
    sidecar.next_event().await.unwrap();

    sidecar
        .send_message(
            "TRIGGER_ATTACHED".to_string(),
            vec![forge_domain::Attachment { path: "/x/a b.png".into(), kind: forge_domain::AttachmentKind::Image }],
        )
        .await
        .unwrap();
    match sidecar.next_event().await.unwrap() {
        SidecarEvent::AssistantDelta { text } => {
            assert_eq!(text, r#"attached:[{"kind":"image","path":"/x/a b.png"}] dirs:["/extra"]"#)
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn recovers_from_a_malformed_line_without_dropping_the_connection() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &[]).unwrap();
    sidecar.start_session(default_params()).await.unwrap();
    sidecar.next_event().await.unwrap();

    sidecar.send_message("TRIGGER_MALFORMED".to_string(), Vec::new()).await.unwrap();

    assert!(matches!(sidecar.next_event().await.unwrap(), SidecarEvent::Error { .. }));
    match sidecar.next_event().await.unwrap() {
        SidecarEvent::AssistantDelta { text } => assert_eq!(text, "recovered-after-malformed"),
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn handles_a_very_large_event_payload() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &[]).unwrap();
    sidecar.start_session(default_params()).await.unwrap();
    sidecar.next_event().await.unwrap();

    sidecar.send_message("TRIGGER_LARGE".to_string(), Vec::new()).await.unwrap();
    match sidecar.next_event().await.unwrap() {
        SidecarEvent::ToolResult { output, .. } => assert_eq!(output.len(), 200_000),
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn interleaves_writes_and_reads_without_deadlock() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &[]).unwrap();
    sidecar.start_session(default_params()).await.unwrap();
    sidecar.next_event().await.unwrap();

    for i in 0..5 {
        sidecar.send_message(format!("msg-{i}"), Vec::new()).await.unwrap();
        match sidecar.next_event().await.unwrap() {
            SidecarEvent::AssistantDelta { text } => assert_eq!(text, format!("echo:msg-{i}")),
            other => panic!("unexpected event: {other:?}"),
        }
    }
}

#[tokio::test]
async fn next_event_returns_none_after_the_process_exits() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &[]).unwrap();
    sidecar.start_session(default_params()).await.unwrap();
    sidecar.next_event().await.unwrap();

    sidecar.shutdown().await.unwrap();
    sidecar.next_event().await.unwrap(); // session_ended
    assert!(sidecar.next_event().await.is_none());
}

#[tokio::test]
async fn spawn_applies_the_given_environment_variables() {
    let env = vec![("FORGE_TEST_VALUE".to_string(), "from-spawn".to_string())];
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &env).unwrap();
    sidecar.start_session(default_params()).await.unwrap();
    sidecar.next_event().await.unwrap();

    sidecar.send_message("TRIGGER_ENV:FORGE_TEST_VALUE".to_string(), Vec::new()).await.unwrap();

    match sidecar.next_event().await.unwrap() {
        SidecarEvent::AssistantDelta { text } => assert_eq!(text, "env:from-spawn"),
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn spawn_without_extra_environment_leaves_the_variable_unset() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &[]).unwrap();
    sidecar.start_session(default_params()).await.unwrap();
    sidecar.next_event().await.unwrap();

    sidecar.send_message("TRIGGER_ENV:FORGE_TEST_UNSET_VALUE".to_string(), Vec::new()).await.unwrap();

    match sidecar.next_event().await.unwrap() {
        SidecarEvent::AssistantDelta { text } => assert_eq!(text, "env:<unset>"),
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn start_session_delivers_the_active_extensions_to_the_sidecar() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &[]).unwrap();
    let mut params = default_params();
    params.extensions = ActiveExtensions {
        skills: BTreeMap::from([("pdf".to_string(), PathBuf::from("/skills/pdf"))]),
        plugin_paths: vec![PathBuf::from("/plugins/hud")],
        mcp_servers: BTreeMap::from([("files".to_string(), serde_json::json!({"command": "npx"}))]),
    };
    sidecar.start_session(params).await.unwrap();
    sidecar.next_event().await.unwrap();

    sidecar.send_message("TRIGGER_EXTENSIONS".to_string(), Vec::new()).await.unwrap();
    match sidecar.next_event().await.unwrap() {
        SidecarEvent::AssistantDelta { text } => {
            let seen: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(
                seen,
                serde_json::json!({
                    "skills": {"pdf": "/skills/pdf"},
                    "plugins": ["/plugins/hud"],
                    "mcp_servers": {"files": {"command": "npx"}}
                })
            );
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn a_session_without_extensions_still_sends_empty_lists() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &[]).unwrap();
    sidecar.start_session(default_params()).await.unwrap();
    sidecar.next_event().await.unwrap();

    sidecar.send_message("TRIGGER_EXTENSIONS".to_string(), Vec::new()).await.unwrap();
    match sidecar.next_event().await.unwrap() {
        SidecarEvent::AssistantDelta { text } => {
            let seen: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(seen, serde_json::json!({"skills": {}, "plugins": [], "mcp_servers": {}}));
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

fn marker_path(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("forge-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

#[tokio::test]
async fn shutdown_returns_only_after_the_process_has_exited_by_itself() {
    let marker = marker_path("exit-marker");
    let env = vec![("FORGE_TEST_EXIT_MARKER".to_string(), marker.to_string_lossy().to_string())];
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &env).unwrap();
    sidecar.start_session(default_params()).await.unwrap();
    sidecar.next_event().await.unwrap();

    sidecar.shutdown().await.unwrap();

    let exists = marker.exists();
    let _ = std::fs::remove_file(&marker);
    assert!(exists, "the sidecar must finish its shutdown work before shutdown() returns");
}

#[tokio::test]
async fn shutdown_kills_a_sidecar_that_ignores_it_after_the_timeout() {
    let env = vec![("FORGE_TEST_IGNORE_SHUTDOWN".to_string(), "1".to_string())];
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &env)
        .unwrap()
        .with_shutdown_timeout(std::time::Duration::from_millis(300));
    sidecar.start_session(default_params()).await.unwrap();
    sidecar.next_event().await.unwrap();

    let started = std::time::Instant::now();
    sidecar.shutdown().await.unwrap();

    assert!(started.elapsed() >= std::time::Duration::from_millis(300));
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    // The process is gone, so its stdout has closed.
    assert!(sidecar.next_event().await.is_none());
}
