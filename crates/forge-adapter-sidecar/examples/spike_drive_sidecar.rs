use forge_adapter_sidecar::SidecarProcess;
use forge_domain::PermissionMode;
use forge_ports::{SidecarEvent, SidecarPort, StartSessionParams};

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let sidecar_script = args
        .next()
        .expect("usage: spike_drive_sidecar <path-to-sidecar/dist/index.js> <cwd>");
    let cwd = args
        .next()
        .expect("usage: spike_drive_sidecar <path-to-sidecar/dist/index.js> <cwd>");

    let sidecar = SidecarProcess::spawn("node", &sidecar_script, &[]).expect("failed to spawn sidecar");

    sidecar
        .start_session(StartSessionParams {
            cwd,
            resume_session_id: None,
            permission_mode: PermissionMode::AcceptEdits,
            model: None,
            effort: None,
            extensions: Default::default(),
            additional_directories: Vec::new(),
            allowed_tools: Vec::new(),
            env: forge_domain::EnvVars::default(),
            defaults: forge_domain::SessionDefaults::default(),
        })
        .await
        .expect("failed to send start_session");

    sidecar
        .send_message("Say hello and create a file named hello.txt with the text 'hi' in it.".to_string(), Vec::new())
        .await
        .expect("failed to send message");

    loop {
        match sidecar.next_event().await {
            Some(event) => {
                println!("{event:?}");
                if matches!(event, SidecarEvent::SessionEnded) {
                    break;
                }
            }
            None => break,
        }
    }
}
