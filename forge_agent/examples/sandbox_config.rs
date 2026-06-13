//! Sandbox configuration example.
//!
//! Demonstrates how to configure tool restrictions and command blocking.
//! Run with: `cargo run --example sandbox_config`

use forge_agent::chat::sandbox::Sandbox;
use forge_agent::chat::tools::builtins::ShellExecTool;
use forge_agent::chat::AsyncTool;

#[tokio::main]
async fn main() {
    let sandbox = Sandbox::new()
        .with_blocked_commands(&[
            "sudo".to_string(),
            "rm\\s+-rf".to_string(),
            "curl.*\\|.*sh".to_string(),
            "wget.*\\|.*sh".to_string(),
        ])
        .with_blocked_paths(&[
            "\\.env".to_string(),
            "id_rsa".to_string(),
            "credentials".to_string(),
        ]);

    let shared = forge_agent::chat::sandbox::shared_sandbox(Some(sandbox));
    let temp = tempfile::tempdir().unwrap();
    let tool = ShellExecTool::new(temp.path()).with_sandbox(shared);

    let safe = tool
        .call(serde_json::json!({"command": "echo hello"}))
        .await;
    println!("echo hello: {:?}", safe.map(|s| s.trim().to_string()));

    let blocked = tool
        .call(serde_json::json!({"command": "sudo apt install evil"}))
        .await;
    println!("sudo apt install evil: {:?}", blocked);

    let blocked2 = tool.call(serde_json::json!({"command": "rm -rf /"})).await;
    println!("rm -rf /: {:?}", blocked2);

    println!("\nSandbox policy applied. Safe commands run, dangerous ones are blocked.");
}
