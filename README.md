# mcplex

**npx for MCP servers** — install the catalog once, pay the context cost only for what you actually load.

[![Crates.io](https://img.shields.io/crates/v/mcplex.svg)](https://crates.io/crates/mcplex)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![CI](https://github.com/Parth3930/mcplex/actions/workflows/ci.yml/badge.svg)](https://github.com/Parth3930/mcplex/actions)

## The Problem

Adding an MCP server to your agent is a one-way door. Every server you add — Supabase, Sentry, Playwright, Betterstack — dumps all of its tool definitions into context permanently, whether you use them once a session or never. Want to add one mid-session? Restart and lose your context. There's no lazy loading, no unloading, no cost visibility. You're paying rent on tools you're not using, every single turn.

`mcplex` is the single MCP server you point your client at instead of N real ones — it shows you a one-line catalog, loads real tools only when an agent asks for them, and lets you hot-swap servers mid-session without losing your conversation.

| | Without mcplex | With mcplex |
|---|---|---|
| **Tools in context at session start** | 40+ (every configured server) | 4 (`list_servers`, `load_server`, `unload_server`, `server_status`) |
| **Add a new server mid-session** | Restart client, lose context | `load_server("name")`, keep going |
| **Know what a server costs you** | No visibility | `server_status` shows per-server token estimate |
| **Idle server you forgot about** | Sits in context forever | Auto-unloads after idle timeout (coming soon) |

## Demo

*(Demo GIF showing `list_servers` -> `load_server("playwright")` -> tool call -> `unload_server` goes here)*

## Install

```bash
cargo install mcplex
```

Then, configure your Claude Desktop or Claude Code client to point to `mcplex`:

```json
{
  "mcpServers": {
    "mcplex": {
      "command": "mcplex",
      "args": ["--config", "/path/to/your/config.toml"]
    }
  }
}
```

## Quickstart Config

Create a `config.toml` file to define your server catalog. You can define as many servers as you want; they cost zero context tokens until loaded.

```toml
[[servers]]
name = "supabase"
description = "Query and manage Supabase projects (tables, RLS, storage, auth)"
command = "npx"
args = ["-y", "@supabase/mcp-server-supabase"]
env = { SUPABASE_ACCESS_TOKEN = "${SUPABASE_ACCESS_TOKEN}" }
auto_unload_after_idle_secs = 600

[[servers]]
name = "playwright"
description = "Browser automation: navigate, click, screenshot"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-playwright"]
```

## How it works

`mcplex` acts as a transparent proxy. When it starts, it advertises only its meta-tools to your client. When you ask it to load a server, it spawns that server as a child process, fetches its tool list, namespaces them (e.g. `playwright.navigate`), and fires a `notifications/tools/list_changed` event. Your client re-fetches the tool list and instantly sees the new tools, mid-session.

## Comparison to Alternatives

There are several other projects in this space, but `mcplex` is the only one offering a full enterprise-grade feature set:

| Feature | GitLab lazy-mcp | mcp-lazy (npm) | voicetreelab/lazy-mcp | lazy-mcp-preload | **mcplex 0.2** |
|---|:---:|:---:|:---:|:---:|:---:|
| Basic lazy load/unload | ✅ | ✅ | ✅ | ✅ | ✅ |
| Config CLI rewriting | ❌ | ✅ | ❌ | ❌ | ✅ (`mcplex add`) |
| Category browsing | ❌ | ❌ | ✅ | ❌ | ✅ |
| Warm preloading | ❌ | ❌ | ❌ | ✅ | ✅ |
| **Shared warm daemon** | ❌ | ❌ | ❌ | ❌ | ✅ (Share servers across IDEs) |
| **Full protocol proxy** | ❌ | ❌ | ❌ | ❌ | ✅ (Resources & Prompts) |
| **Real tokenizer costs** | ❌ | ❌ | ❌ | ❌ | ✅ (tiktoken-rs cl100k) |
| **Live Web Dashboard** | ❌ | ❌ | ❌ | ❌ | ✅ (Port 4124) |
| **Adaptive Eviction** | ❌ | ❌ | ❌ | ❌ | ✅ (`max_total_tokens`) |
| **Safety dry-run** | ❌ | ❌ | ❌ | ❌ | ✅ (Secret redaction) |
| **Permission Hooks** | ❌ | ❌ | ❌ | ❌ | ✅ |
| **Config merging** | ❌ | ❌ | ❌ | ❌ | ✅ (`conf.d/` dir support) |

## Meta-Tools Reference

| Tool | Description |
|---|---|
| `list_servers` | Returns the catalog of available servers and their load status. |
| `load_server(name)` | Spawns the real MCP server and merges its tools into the client's context. |
| `unload_server(name)` | Tears down the child connection and removes its tools from context. |
| `server_status` | Shows uptime and real tiktoken token footprint for each loaded server. |
| `browse_category(path)` | Navigate large server lists hierarchically. |

## Dashboard & Daemon Mode
Instead of running separate instances for Claude Desktop, Claude Code, and Cursor, start the `mcplex daemon`:
```bash
mcplex daemon
```
This runs a shared daemon on `127.0.0.1:4123` and a live web dashboard on `http://127.0.0.1:4124`. Your clients automatically act as thin proxies, sharing the warm server pool to save memory and tokens!

## Roadmap

Planned for future releases:
- Semantic auto-loading (automatically loading a server based on intent).
- Remote/HTTP transport (SSE/WebSocket proxying).
- OAuth 2.0 + PKCE for remote servers.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for details. 
**Good first issue:** Add a popular MCP server to our `config.example.toml` catalog! It requires zero Rust knowledge and helps everyone.

## License

MIT License.
