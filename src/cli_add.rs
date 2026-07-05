use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use crate::config::{Config, ServerSpec};

pub fn get_client_config_path(claude_desktop: bool, claude_code: bool, cursor: bool, custom_path: Option<String>) -> Result<PathBuf, String> {
    if let Some(path) = custom_path {
        return Ok(PathBuf::from(path));
    }

    if claude_desktop {
        if cfg!(target_os = "windows") {
            if let Some(mut path) = dirs::data_dir() {
                path.push("Claude");
                path.push("claude_desktop_config.json");
                return Ok(path);
            }
        } else if cfg!(target_os = "macos") {
            if let Some(mut path) = dirs::home_dir() {
                path.push("Library");
                path.push("Application Support");
                path.push("Claude");
                path.push("claude_desktop_config.json");
                return Ok(path);
            }
        } else {
            if let Some(mut path) = dirs::config_dir() {
                path.push("Claude");
                path.push("claude_desktop_config.json");
                return Ok(path);
            }
        }
    }

    if claude_code {
        return Ok(PathBuf::from(".claude.json")); // Current directory or we can search up
    }

    if cursor {
        if cfg!(target_os = "windows") {
            if let Some(mut path) = dirs::data_dir() {
                path.push("Cursor");
                path.push("User");
                path.push("global_workspace.json"); // Approximate
                return Ok(path);
            }
        } else if cfg!(target_os = "macos") {
            if let Some(mut path) = dirs::home_dir() {
                path.push("Library");
                path.push("Application Support");
                path.push("Cursor");
                path.push("User");
                path.push("global_workspace.json");
                return Ok(path);
            }
        } else {
            if let Some(mut path) = dirs::config_dir() {
                path.push("Cursor");
                path.push("User");
                path.push("global_workspace.json");
                return Ok(path);
            }
        }
    }

    Err("Must specify either --claude-desktop, --claude-code, --cursor, or a path".into())
}

pub fn execute_add(client_config_path: &Path, lazy_config_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Reading client config from: {}", client_config_path.display());
    let client_config_content = fs::read_to_string(client_config_path)?;
    let mut client_json: Value = serde_json::from_str(&client_config_content)?;

    let mut lazy_config = if Path::new(lazy_config_path).exists() {
        crate::config::load_config(lazy_config_path).unwrap_or(Config { max_total_tokens: None, servers: vec![] })
    } else {
        Config { max_total_tokens: None, servers: vec![] }
    };

    if let Some(mcp_servers) = client_json.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        let mut servers_migrated = 0;
        let mut keys_to_remove = Vec::new();
        
        for (name, server_def) in mcp_servers.iter() {
            if name == "mcplex" {
                continue;
            }
            
            let command = server_def.get("command").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let mut args = vec![];
            if let Some(args_arr) = server_def.get("args").and_then(|v| v.as_array()) {
                for arg in args_arr {
                    if let Some(arg_str) = arg.as_str() {
                        args.push(arg_str.to_string());
                    }
                }
            }
            let mut env = std::collections::HashMap::new();
            if let Some(env_obj) = server_def.get("env").and_then(|v| v.as_object()) {
                for (k, v) in env_obj {
                    if let Some(v_str) = v.as_str() {
                        env.insert(k.clone(), v_str.to_string());
                    }
                }
            }

            if !lazy_config.servers.iter().any(|s| s.name == *name) {
                lazy_config.servers.push(ServerSpec {
                    name: name.clone(),
                    description: Some(format!("Imported from {}", client_config_path.display())),
                    preload_on_start: None,
                    oauth_auth_url: None,
                    oauth_token_url: None,
                    command,
                    args,
                    env,
                    auto_unload_after_idle_secs: None,
                    permission_hook: None,
                });
                servers_migrated += 1;
            }
            keys_to_remove.push(name.clone());
        }
        
        for k in keys_to_remove {
            mcp_servers.remove(&k);
        }

        let lazy_mcp_args = vec!["--config".to_string(), lazy_config_path.to_string()];
        
        mcp_servers.insert("mcplex".to_string(), serde_json::json!({
            "command": "mcplex",
            "args": lazy_mcp_args
        }));

        println!("Migrated {} servers to {}", servers_migrated, lazy_config_path);
    } else {
        return Err("No mcpServers found in config".into());
    }

    let lazy_config_toml = toml::to_string(&lazy_config)?;
    fs::write(lazy_config_path, lazy_config_toml)?;

    let new_client_config = serde_json::to_string_pretty(&client_json)?;
    fs::write(client_config_path, new_client_config)?;

    println!("Successfully updated client config to point to lazy-mcp.");
    Ok(())
}
