use forge_adapter_sidecar::SidecarProcess;
use forge_domain::PermissionMode;
use forge_ports::{SidecarEvent, SidecarPort, StartSessionParams};

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
    }
}

#[tokio::test]
async fn start_session_and_send_message_round_trip() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &[]).unwrap();

    sidecar.start_session(default_params()).await.unwrap();
    let started = sidecar.next_event().await.unwrap();
    assert!(matches!(started, SidecarEvent::SessionStarted { .. }));

    sidecar.send_message("hello".to_string()).await.unwrap();
    match sidecar.next_event().await.unwrap() {
        SidecarEvent::AssistantDelta { text } => assert_eq!(text, "echo:hello"),
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn recovers_from_a_malformed_line_without_dropping_the_connection() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &[]).unwrap();
    sidecar.start_session(default_params()).await.unwrap();
    sidecar.next_event().await.unwrap();

    sidecar.send_message("TRIGGER_MALFORMED".to_string()).await.unwrap();

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

    sidecar.send_message("TRIGGER_LARGE".to_string()).await.unwrap();
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
        sidecar.send_message(format!("msg-{i}")).await.unwrap();
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

    sidecar.send_message("TRIGGER_ENV:FORGE_TEST_VALUE".to_string()).await.unwrap();

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

    sidecar.send_message("TRIGGER_ENV:FORGE_TEST_UNSET_VALUE".to_string()).await.unwrap();

    match sidecar.next_event().await.unwrap() {
        SidecarEvent::AssistantDelta { text } => assert_eq!(text, "env:<unset>"),
        other => panic!("unexpected event: {other:?}"),
    }
}
