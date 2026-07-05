mod child;
mod cli_add;
mod config;
mod gateway;
mod registry;
mod stats;

use clap::{Parser, Subcommand};
use gateway::Gateway;
use rmcp::ServiceExt;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start shared warm daemon
    Daemon,
    /// Certify servers from a config
    Certify {
        #[arg(short, long, default_value = "config.toml")]
        config: String,
    },
    /// Add existing servers from a client's config
    Add {
        /// Target Claude Desktop config
        #[arg(long)]
        claude_desktop: bool,

        /// Target Claude Code config
        #[arg(long)]
        claude_code: bool,

        /// Target Cursor config
        #[arg(long)]
        cursor: bool,

        /// Explicit path to client config
        path: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.command {
        Some(Commands::Certify { config }) => {
            let conf = config::load_config(&config).unwrap_or_else(|e| {
                eprintln!("Failed to load config: {}", e);
                std::process::exit(1);
            });
            let registry = Arc::new(Mutex::new(registry::Registry::new(conf)));
            let servers: Vec<String> = registry.lock().await.catalog.keys().cloned().collect();
            for server in servers {
                println!("Certifying {}...", server);
                let mut reg = registry.lock().await;
                match reg.load_server(&server).await {
                    Ok(_) => println!("✅ {} loaded successfully", server),
                    Err(e) => println!("❌ {} failed: {}", server, e),
                }
            }
            return Ok(());
        }
        Some(Commands::Add {
            claude_desktop,
            claude_code,
            cursor,
            path,
        }) => {
            let client_path =
                cli_add::get_client_config_path(claude_desktop, claude_code, cursor, path)?;
            cli_add::execute_add(&client_path, &args.config)?;
            return Ok(());
        }
        Some(Commands::Daemon) => {
            let conf = config::load_config(&args.config).unwrap_or_else(|e| {
                eprintln!("Failed to load config: {}", e);
                config::Config {
                    max_total_tokens: None,
                    servers: vec![],
                }
            });

            let reg = registry::Registry::new(conf);

            // Check for preload_on_start
            let preload_servers: Vec<_> = reg
                .catalog
                .values()
                .filter_map(|s| {
                    if s.preload_on_start.unwrap_or(false) {
                        Some(s.name.clone())
                    } else {
                        None
                    }
                })
                .collect();

            let registry = Arc::new(Mutex::new(reg));

            for name in preload_servers {
                eprintln!("Preloading server: {}", name);
                let mut reg_lock = registry.lock().await;
                if let Err(e) = reg_lock.load_server(&name).await {
                    eprintln!("Failed to preload {}: {}", name, e);
                }
            }

            let registry_for_web = Arc::clone(&registry);
            tokio::spawn(async move {
                use axum::{Router, routing::get};
                let app = Router::new().route(
                    "/",
                    get({
                        let reg = Arc::clone(&registry_for_web);
                        move || async move {
                            let reg_lock = reg.lock().await;
                            let loaded_count = reg_lock.loaded.len();
                            let total_tokens: usize = reg_lock.loaded.values().map(|s| s.tools.iter().map(crate::stats::count_tokens).sum::<usize>()).sum();
                            axum::response::Html(format!(
                                "<h1>mcplex Dashboard</h1><p>Loaded Servers: {}</p><p>Total Tokens: {}</p>",
                                loaded_count, total_tokens
                            ))
                        }
                    }),
                );
                let listener = tokio::net::TcpListener::bind("127.0.0.1:4124")
                    .await
                    .unwrap();
                eprintln!("Dashboard listening on http://127.0.0.1:4124");
                axum::serve(listener, app).await.unwrap();
            });

            let listener = TcpListener::bind("127.0.0.1:4123").await?;
            eprintln!("Daemon listening on 127.0.0.1:4123");

            loop {
                let (stream, _) = listener.accept().await?;
                let (read_half, write_half) = tokio::io::split(stream);
                let gateway = Gateway {
                    registry: Arc::clone(&registry),
                };
                tokio::spawn(async move {
                    if let Ok(service) = gateway.serve((read_half, write_half)).await {
                        let _ = service.waiting().await;
                    }
                });
            }
        }
        None => {
            let stream = match TcpStream::connect("127.0.0.1:4123").await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Failed to connect to daemon: {}", e);
                    std::process::exit(1);
                }
            };
            let (mut read_half, mut write_half) = tokio::io::split(stream);
            let mut stdin = tokio::io::stdin();
            let mut stdout = tokio::io::stdout();

            let c1 = tokio::io::copy(&mut stdin, &mut write_half);
            let c2 = tokio::io::copy(&mut read_half, &mut stdout);

            let _ = tokio::try_join!(c1, c2);
        }
    }

    Ok(())
}
