use crate::child::spawn_child;
use crate::registry::Registry;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler, model::*, service::RequestContext};
use serde_json::Value;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct Gateway {
    pub registry: Arc<Mutex<Registry>>,
}

fn to_schema(v: Value) -> Arc<serde_json::Map<String, Value>> {
    Arc::new(v.as_object().unwrap().clone())
}

impl ServerHandler for Gateway {
    fn get_info(&self) -> InitializeResult {
        InitializeResult {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
            server_info: Implementation {
                name: "mcp-warmpool".into(),
                version: "0.1.0".into(),
                description: None,
                icons: None,
                title: None,
                website_url: None,
            },
            instructions: None,
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        async move {
            let reg = self.registry.lock().await;

            let mut tools = vec![
                Tool {
                    name: "list_servers".into(),
                    title: None,
                    description: Some("Returns the catalog of available servers".into()),
                    input_schema: to_schema(serde_json::json!({
                        "type": "object",
                        "properties": {}
                    })),
                    output_schema: None,
                    annotations: None,
                    execution: None,
                    icons: None,
                    meta: None,
                },
                Tool {
                    name: "load_server".into(),
                    title: None,
                    description: Some("Spawns/connects the real MCP server".into()),
                    input_schema: to_schema(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" }
                        },
                        "required": ["name"]
                    })),
                    output_schema: None,
                    annotations: None,
                    execution: None,
                    icons: None,
                    meta: None,
                },
                Tool {
                    name: "unload_server".into(),
                    title: None,
                    description: Some("Tears down the child connection".into()),
                    input_schema: to_schema(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" }
                        },
                        "required": ["name"]
                    })),
                    output_schema: None,
                    annotations: None,
                    execution: None,
                    icons: None,
                    meta: None,
                },
                Tool {
                    name: "server_status".into(),
                    title: None,
                    description: Some("Uptime and token accounting".into()),
                    input_schema: to_schema(serde_json::json!({
                        "type": "object",
                        "properties": {}
                    })),
                    output_schema: None,
                    annotations: None,
                    execution: None,
                    icons: None,
                    meta: None,
                },
                Tool {
                    name: "browse_category".into(),
                    title: None,
                    description: Some("Hierarchical category browsing".into()),
                    input_schema: to_schema(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" }
                        },
                        "required": ["path"]
                    })),
                    output_schema: None,
                    annotations: None,
                    execution: None,
                    icons: None,
                    meta: None,
                },
            ];

            for (server_name, loaded) in reg.loaded.iter() {
                for tool in &loaded.tools {
                    let mut namespaced = tool.clone();
                    namespaced.name = format!("{}.{}", server_name, tool.name).into();
                    tools.push(namespaced);
                }
            }

            Ok(ListToolsResult {
                tools,
                next_cursor: None,
                meta: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        async move {
            if request.name == "list_servers" {
                let reg = self.registry.lock().await;
                let mut servers = Vec::new();
                for (name, spec) in &reg.catalog {
                    servers.push(serde_json::json!({
                        "name": name,
                        "description": spec.description,
                        "loaded": reg.loaded.contains_key(name),
                    }));
                }
                return Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&servers).unwrap(),
                )]));
            }

            if request.name == "browse_category" {
                let path = request
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("path"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::invalid_params("Missing path".to_string(), None))?;

                let reg = self.registry.lock().await;
                if path.is_empty() {
                    let mut categories: Vec<String> = reg.catalog.keys().cloned().collect();
                    categories.sort();
                    return Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string_pretty(&categories).unwrap(),
                    )]));
                } else {
                    let server_name = path;
                    if let Some(loaded) = reg.loaded.get(server_name) {
                        let mut tools: Vec<String> = loaded.tools.iter().map(|t| t.name.to_string()).collect();
                        tools.sort();
                        return Ok(CallToolResult::success(vec![Content::text(
                            serde_json::to_string_pretty(&tools).unwrap(),
                        )]));
                    } else if reg.catalog.contains_key(server_name) {
                        return Ok(CallToolResult::success(vec![Content::text(format!(
                            "Server {} is not loaded, but exists in catalog.",
                            server_name
                        ))]));
                    } else {
                        return Err(McpError::invalid_params(
                            format!("Category (server) {} not found", server_name),
                            None,
                        ));
                    }
                }
            }

            if request.name == "server_status" {
                let reg = self.registry.lock().await;
                let mut status = Vec::new();
                for (name, loaded) in &reg.loaded {
                    let tokens: usize =
                        loaded.tools.iter().map(crate::stats::count_tokens).sum();
                    status.push(serde_json::json!({
                        "name": name,
                        "tools": loaded.tools.len(),
                        "real_tokens": tokens,
                    }));
                }
                return Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&status).unwrap(),
                )]));
            }

            if request.name == "load_server" {
                let name = request
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("name"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::invalid_params("Missing name".to_string(), None))?
                    .to_string();

                let mut reg = self.registry.lock().await;
                if reg.loaded.contains_key(&name) {
                    return Ok(CallToolResult::success(vec![Content::text(format!(
                        "Server {} already loaded",
                        name
                    ))]));
                }

                if let Some(spec) = reg.catalog.get(&name) {
                    let (handle, tools) = spawn_child(spec).await.map_err(|e| {
                        McpError::internal_error(format!("Failed to spawn child: {}", e), None)
                    })?;

                    reg.loaded.insert(
                        name.clone(),
                        crate::registry::LoadedServer {
                            handle,
                            tools,
                            last_used: std::time::Instant::now(),
                        },
                    );

                    let _ = context.peer.notify_tool_list_changed().await;
                    return Ok(CallToolResult::success(vec![Content::text(format!(
                        "Server {} loaded successfully",
                        name
                    ))]));
                } else {
                    return Err(McpError::invalid_params(
                        format!("Server {} not found in config", name),
                        None,
                    ));
                }
            }

            if request.name == "unload_server" {
                let name = request
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("name"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::invalid_params("Missing name".to_string(), None))?
                    .to_string();

                let mut reg = self.registry.lock().await;
                if let Some(_loaded) = reg.loaded.remove(&name) {
                    // Let the service handle be dropped, wait, let's just let it drop.
                    // loaded.handle.service is a RunningService, dropping it drops the connection? No, we might need to drop it.
                    let _ = context.peer.notify_tool_list_changed().await;
                    return Ok(CallToolResult::success(vec![Content::text(format!(
                        "Server {} unloaded successfully",
                        name
                    ))]));
                } else {
                    return Err(McpError::invalid_params(
                        format!("Server {} is not loaded", name),
                        None,
                    ));
                }
            }

            let parts: Vec<&str> = request.name.splitn(2, '.').collect();
            if parts.len() != 2 {
                return Err(McpError::invalid_params(
                    "Invalid tool name format".to_string(),
                    None,
                ));
            }
            let server_name = parts[0];
            let tool_name = parts[1];

            let (service, permission_hook) = {
                let mut reg = self.registry.lock().await;
                
                let hook = reg.catalog.get(server_name).and_then(|s| s.permission_hook.clone());

                if let Some(loaded) = reg.loaded.get_mut(server_name) {
                    loaded.last_used = std::time::Instant::now();
                    (loaded.handle.service.clone(), hook)
                } else {
                    return Err(McpError::invalid_params(
                        format!("Server {} not loaded", server_name),
                        None,
                    ));
                }
            };

            if let Some(hook) = permission_hook {
                let status = tokio::process::Command::new(&hook)
                    .arg(format!("{}.{}", server_name, tool_name))
                    .status()
                    .await;
                match status {
                    Ok(exit_status) if exit_status.success() => {} // allowed
                    Ok(_) => {
                        return Err(McpError::internal_error(
                            format!("Permission denied by hook for {}.{}", server_name, tool_name),
                            None,
                        ));
                    }
                    Err(e) => {
                        return Err(McpError::internal_error(
                            format!("Failed to execute permission hook: {}", e),
                            None,
                        ));
                    }
                }
            }

            let mut req = request.clone();
            req.name = tool_name.to_string().into();

            // ponytail: simple dry_run check, skip downstream
            if let Some(args) = &req.arguments
                && let Some(dry_run) = args.get("dry_run")
                    && dry_run.as_bool() == Some(true) {
                        let mut args_clone = args.clone();
                        for (k, v) in args_clone.iter_mut() {
                            let key_lower = k.to_lowercase();
                            if key_lower.contains("secret") || key_lower.contains("password") || key_lower.contains("token") || key_lower.contains("key") {
                                *v = serde_json::json!("[REDACTED]");
                            }
                        }
                        return Ok(CallToolResult::success(vec![Content::text(
                            serde_json::to_string_pretty(&args_clone).unwrap_or_default()
                        )]));
                    }

            match service.call_tool(req).await {
                Ok(res) => Ok(res),
                Err(e) => Err(McpError::internal_error(
                    format!("Downstream error: {:?}", e),
                    None,
                )),
            }
        }
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, McpError>> + Send + '_ {
        async move {
            let reg = self.registry.lock().await;
            let mut all_resources = Vec::new();

            for (server_name, loaded) in reg.loaded.iter() {
                if let Ok(res) = loaded.handle.service.list_resources(Default::default()).await {
                    for mut resource in res.resources {
                        resource.uri = format!("{}://{}", server_name, resource.uri).into();
                        all_resources.push(resource);
                    }
                }
            }

            Ok(ListResourcesResult {
                resources: all_resources,
                next_cursor: None,
                meta: None,
            })
        }
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ReadResourceResult, McpError>> + Send + '_ {
        async move {
            let uri_str = request.uri.as_str();
            let parts: Vec<&str> = uri_str.splitn(2, "://").collect();
            if parts.len() != 2 {
                return Err(McpError::invalid_params("Invalid resource URI format".to_string(), None));
            }
            let server_name = parts[0];
            let original_uri = parts[1];

            let service = {
                let reg = self.registry.lock().await;
                if let Some(loaded) = reg.loaded.get(server_name) {
                    loaded.handle.service.clone()
                } else {
                    return Err(McpError::invalid_params(format!("Server {} not loaded", server_name), None));
                }
            };

            let mut req = request.clone();
            req.uri = original_uri.to_string().into();

            service.read_resource(req).await.map_err(|e| McpError::internal_error(format!("Downstream error: {:?}", e), None))
        }
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, McpError>> + Send + '_ {
        async move {
            let reg = self.registry.lock().await;
            let mut all_prompts = Vec::new();

            for (server_name, loaded) in reg.loaded.iter() {
                if let Ok(res) = loaded.handle.service.list_prompts(Default::default()).await {
                    for mut prompt in res.prompts {
                        prompt.name = format!("{}.{}", server_name, prompt.name).into();
                        all_prompts.push(prompt);
                    }
                }
            }

            Ok(ListPromptsResult {
                prompts: all_prompts,
                next_cursor: None,
                meta: None,
            })
        }
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<GetPromptResult, McpError>> + Send + '_ {
        async move {
            let name_str = request.name.to_string();
            let parts: Vec<&str> = name_str.splitn(2, '.').collect();
            if parts.len() != 2 {
                return Err(McpError::invalid_params("Invalid prompt name format".to_string(), None));
            }
            let server_name = parts[0];
            let prompt_name = parts[1];

            let service = {
                let reg = self.registry.lock().await;
                if let Some(loaded) = reg.loaded.get(server_name) {
                    loaded.handle.service.clone()
                } else {
                    return Err(McpError::invalid_params(format!("Server {} not loaded", server_name), None));
                }
            };

            let mut req = request.clone();
            req.name = prompt_name.to_string().into();

            service.get_prompt(req).await.map_err(|e| McpError::internal_error(format!("Downstream error: {:?}", e), None))
        }
    }
}
