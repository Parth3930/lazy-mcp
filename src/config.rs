use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSpec {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub preload_on_start: Option<bool>,
    pub oauth_auth_url: Option<String>,
    pub oauth_token_url: Option<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    pub auto_unload_after_idle_secs: Option<u64>,
    pub permission_hook: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub max_total_tokens: Option<usize>,
    #[serde(default)]
    pub servers: Vec<ServerSpec>,
}

pub fn load_config(path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let mut config: Config = toml::from_str(&content)?;

    // Scan for conf.d directory
    let dir_path = format!("{}.d", path);
    if let Ok(entries) = std::fs::read_dir(&dir_path) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(mut extra_config) = toml::from_str::<Config>(&content) {
                                config.servers.append(&mut extra_config.servers);
                            }
                        }
                    }
                }
            }
        }
    }

    for server in &mut config.servers {
        for value in server.env.values_mut() {
            let mut new_value = value.clone();
            while let Some(start) = new_value.find("${") {
                if let Some(end_rel) = new_value[start..].find('}') {
                    let end = start + end_rel;
                    let var_name = &new_value[start + 2..end];
                    let env_val = std::env::var(var_name).unwrap_or_default();
                    new_value.replace_range(start..=end, &env_val);
                } else {
                    break;
                }
            }
            *value = new_value;
        }
    }
    Ok(config)
}
