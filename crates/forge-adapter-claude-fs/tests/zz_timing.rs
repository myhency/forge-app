use forge_adapter_claude_fs::{ClaudeProjectAdapter, ClaudeSessionAdapter};
use forge_ports::{ProjectRepositoryPort, SessionRepositoryPort};
use std::time::Instant;

#[tokio::test]
async fn timing() {
    let home = std::path::PathBuf::from(std::env::var("HOME").unwrap()).join(".claude");
    let projects = ClaudeProjectAdapter::new(home.clone());
    let sessions = ClaudeSessionAdapter::new(home);
    let list = projects.list_projects().await.unwrap();
    let t = Instant::now();
    for p in &list { let _ = sessions.list_sessions(&p.path).await; }
    println!("TIMING sequential list_sessions: {:?}", t.elapsed());
}
