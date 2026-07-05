use rmcp::{transport::TokioChildProcess, ServiceExt};
use tokio::process::Command;

pub struct ChildHandle {
    pub service: rmcp::service::RunningService<rmcp::RoleClient, ()>,
}

pub async fn spawn_child(
    spec: &crate::config::ServerSpec,
) -> Result<(ChildHandle, Vec<rmcp::model::Tool>), Box<dyn std::error::Error>> {
    let mut command = Command::new(&spec.command);
    
    command.args(&spec.args);
    command.envs(&spec.env);

    let service = ().serve(TokioChildProcess::new(command)?).await?;
    let tools = service.list_tools(Default::default()).await?.tools;

    Ok((ChildHandle { service }, tools))
}
