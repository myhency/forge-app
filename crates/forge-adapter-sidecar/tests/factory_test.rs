use std::sync::Arc;

use forge_adapter_sidecar::SidecarProcessFactory;
use forge_domain::{AppConfig, PermissionMode};
use forge_ports::{
    ConfigError, MockConfigRepositoryPort, SidecarError, SidecarEvent, SidecarFactoryPort,
    SidecarPort, StartSessionParams,
};

fn fixture_path() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fake_sidecar.js").to_string()
}

fn node_binary() -> String {
    std::env::var("NODE_BINARY").unwrap_or_else(|_| "node".to_string())
}

fn factory_with(config: MockConfigRepositoryPort) -> SidecarProcessFactory {
    SidecarProcessFactory::new(node_binary(), fixture_path(), Arc::new(config))
}

async fn start(sidecar: &dyn SidecarPort) {
    sidecar
        .start_session(StartSessionParams {
            cwd: "/tmp".to_string(),
            resume_session_id: None,
            permission_mode: PermissionMode::Default,
            model: None,
            extensions: Default::default(),
            additional_directories: Vec::new(),
        })
        .await
        .unwrap();
    assert!(matches!(
        sidecar.next_event().await.unwrap(),
        SidecarEvent::SessionStarted { .. }
    ));
}

// Asks the fixture sidecar to echo one of its own environment variables.
async fn ask_env(sidecar: &dyn SidecarPort, name: &str) -> String {
    sidecar.send_message(format!("TRIGGER_ENV:{name}"), Vec::new()).await.unwrap();
    match sidecar.next_event().await.unwrap() {
        SidecarEvent::AssistantDelta { text } => text,
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn spawn_injects_the_configured_credentials_into_the_sidecar_environment() {
    let mut config = MockConfigRepositoryPort::new();
    config.expect_load().returning(|| {
        Ok(AppConfig {
            api_key: Some("sk-test-123".to_string()),
            base_url: Some("https://proxy.example.test".to_string()),
            ..AppConfig::default()
        })
    });
    let factory = factory_with(config);

    let sidecar = factory.spawn().await.unwrap();

    start(sidecar.as_ref()).await;
    assert_eq!(ask_env(sidecar.as_ref(), "ANTHROPIC_API_KEY").await, "env:sk-test-123");
    assert_eq!(
        ask_env(sidecar.as_ref(), "ANTHROPIC_BASE_URL").await,
        "env:https://proxy.example.test"
    );
}

#[tokio::test]
async fn spawn_reloads_the_config_every_time_so_edits_apply_to_the_next_session() {
    let mut config = MockConfigRepositoryPort::new();
    let mut calls = 0;
    config.expect_load().times(2).returning(move || {
        calls += 1;
        Ok(AppConfig {
            api_key: Some(format!("key-{calls}")),
            ..AppConfig::default()
        })
    });
    let factory = factory_with(config);

    let first = factory.spawn().await.unwrap();
    let second = factory.spawn().await.unwrap();

    start(first.as_ref()).await;
    start(second.as_ref()).await;
    assert_eq!(ask_env(first.as_ref(), "ANTHROPIC_API_KEY").await, "env:key-1");
    assert_eq!(ask_env(second.as_ref(), "ANTHROPIC_API_KEY").await, "env:key-2");
}

#[tokio::test]
async fn spawn_with_a_default_config_still_starts_a_working_sidecar() {
    let mut config = MockConfigRepositoryPort::new();
    config.expect_load().returning(|| Ok(AppConfig::default()));
    let factory = factory_with(config);

    let sidecar = factory.spawn().await.unwrap();

    start(sidecar.as_ref()).await;
    assert_eq!(ask_env(sidecar.as_ref(), "FORGE_TEST_UNSET_VALUE").await, "env:<unset>");
}

#[tokio::test]
async fn spawn_fails_with_spawn_failed_when_the_config_cannot_be_loaded() {
    let mut config = MockConfigRepositoryPort::new();
    config
        .expect_load()
        .returning(|| Err(ConfigError::ParseFailed("bad json".to_string())));
    let factory = factory_with(config);

    let Err(error) = factory.spawn().await else {
        panic!("spawn should fail when the config cannot be loaded");
    };

    match error {
        SidecarError::SpawnFailed(message) => {
            assert!(message.contains("failed to load the Forge config"), "unexpected message: {message}");
            assert!(message.contains("bad json"), "unexpected message: {message}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn spawn_fails_with_spawn_failed_when_the_node_binary_does_not_exist() {
    let mut config = MockConfigRepositoryPort::new();
    config.expect_load().returning(|| Ok(AppConfig::default()));
    let factory = SidecarProcessFactory::new(
        "forge-test-no-such-node-binary".to_string(),
        fixture_path(),
        Arc::new(config),
    );

    let result = factory.spawn().await;

    assert!(matches!(result, Err(SidecarError::SpawnFailed(_))));
}
