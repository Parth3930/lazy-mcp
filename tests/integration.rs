use rmcp::{ServiceExt, transport::TokioChildProcess};
use tokio::process::Command;

#[tokio::test]
async fn test_gateway_lists_meta_tools() -> Result<(), Box<dyn std::error::Error>> {
    let mut daemon_cmd = Command::new(env!("CARGO_BIN_EXE_mcplex"));
    daemon_cmd.arg("--config").arg("config.example.toml").arg("daemon");
    let mut daemon_child = daemon_cmd.spawn()?;
    
    // Give daemon time to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let cmd = Command::new(env!("CARGO_BIN_EXE_mcplex"));
    // Thin client doesn't need --config, but it ignores it
    let transport = TokioChildProcess::new(cmd)?;
    let service = ().serve(transport).await?;

    let tools_res = service.list_tools(Default::default()).await?;
    let tool_names: Vec<String> = tools_res
        .tools
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();

    assert!(tool_names.contains(&"list_servers".to_string()));
    assert!(tool_names.contains(&"load_server".to_string()));
    assert!(tool_names.contains(&"unload_server".to_string()));
    assert!(tool_names.contains(&"server_status".to_string()));

    let call_res = service
        .call_tool(rmcp::model::CallToolRequestParams {
            name: "list_servers".into(),
            arguments: None,
            meta: None,
            task: None,
        })
        .await?;

    let json = serde_json::to_string(&call_res)?;
    assert!(json.contains("playwright"));
    assert!(json.contains("supabase"));

    daemon_child.kill().await?;
    
    Ok(())
}
