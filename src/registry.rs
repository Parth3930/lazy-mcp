use std::collections::HashMap;
use std::time::Instant;

use crate::child::ChildHandle;
use crate::config::{Config, ServerSpec};
use rmcp::model::Tool;

pub struct LoadedServer {
    pub handle: ChildHandle,
    pub tools: Vec<Tool>,
    pub last_used: Instant,
}

pub struct Registry {
    pub config: Config,
    pub catalog: HashMap<String, ServerSpec>,
    pub loaded: HashMap<String, LoadedServer>,
}

impl Registry {
    pub fn new(config: Config) -> Self {
        let catalog = config
            .servers
            .iter()
            .map(|s| (s.name.clone(), s.clone()))
            .collect();
        Self {
            config,
            catalog,
            loaded: HashMap::new(),
        }
    }

    pub async fn load_server(&mut self, name: &str) -> Result<(), String> {
        if self.loaded.contains_key(name) {
            return Ok(());
        }
        let spec = self.catalog.get(name).ok_or_else(|| format!("Server {} not found", name))?;
        let (handle, tools) = crate::child::spawn_child(spec).await.map_err(|e| e.to_string())?;
        self.loaded.insert(
            name.to_string(),
            LoadedServer {
                handle,
                tools,
                last_used: std::time::Instant::now(),
            },
        );

        if let Some(max_tokens) = self.config.max_total_tokens {
            let total_tokens: usize = self.loaded.values().map(|s| s.tools.iter().map(crate::stats::count_tokens).sum::<usize>()).sum();
            if total_tokens > max_tokens {
                if let Some(first_key) = self.loaded.keys().next().cloned() {
                    self.loaded.remove(&first_key);
                }
            }
        }
        
        Ok(())
    }
}
