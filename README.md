# Garudust

A self-improving AI agent written in Rust — inspired by [Hermes Agent](https://github.com/NousResearch/hermes-agent), rebuilt from the ground up for performance and a minimal footprint.

## Features

- **Multi-provider LLM support** — OpenAI-compatible endpoints (OpenRouter, LM Studio, Ollama, …) and Anthropic direct, switchable via env var, no code changes
- **Tool system** — extensible registry; built-in tools for file I/O, terminal execution, web fetch, memory, and skills
- **Skills** — load SKILL.md files from `~/.garudust/skills/`; index injected into every system prompt automatically
- **Persistent memory** — `MEMORY.md` and `USER.md` under `~/.garudust/memories/`, updated by the agent via the `memory` tool
- **Context compressor** — automatically summarizes old conversation turns before the context window fills up
- **Parallel tool dispatch** — multiple tool calls in one turn run concurrently via `tokio::join_all`
- **Interactive TUI** — `ratatui`-based terminal UI with message history, status bar, and input box
- **Messaging platforms** — Telegram and Discord adapters (feature-gated); more can be added by implementing `PlatformAdapter`
- **Cron scheduler** — `tokio-cron-scheduler` wrapper for unattended recurring tasks
- **HTTP gateway** — `axum` server with `/health` endpoint; session store backed by SQLite WAL + FTS5

## Crate Layout

```
crates/
  garudust-core        traits, types, errors — no I/O, depended on by everything
  garudust-transport   LLM adapters (OpenAI-compat, Anthropic)
  garudust-tools       tool registry + built-in toolsets
  garudust-memory      file-backed memory store + SQLite session DB
  garudust-agent       agent loop, context compressor, prompt builder
  garudust-platforms   platform adapters (Telegram, Discord)
  garudust-cron        cron scheduler
  garudust-gateway     axum HTTP gateway, session registry, platform handler

bin/
  garudust             CLI binary — one-shot and interactive TUI
  garudust-server      headless server — starts platform adapters + HTTP gateway
```

## Quick Start

### Prerequisites

- Rust 1.75+
- An API key from [OpenRouter](https://openrouter.ai) (or any OpenAI-compatible provider) or Anthropic

### Install

```bash
git clone https://github.com/yourname/garudust
cd garudust
cargo build --release
```

### Run

**One-shot task**

```bash
OPENROUTER_API_KEY=sk-or-... cargo run -p garudust -- "list the files in the current directory"
```

**Interactive TUI**

```bash
OPENROUTER_API_KEY=sk-or-... cargo run -p garudust
```

Keys: `Enter` to send · `↑ ↓` to scroll · `Ctrl+C` to quit

**Use Anthropic directly**

```bash
ANTHROPIC_API_KEY=sk-ant-... cargo run -p garudust -- --model claude-sonnet-4-6 "hello"
```

**Headless server with Telegram**

```bash
OPENROUTER_API_KEY=sk-or-... \
TELEGRAM_TOKEN=123456:ABC... \
cargo run -p garudust-server
```

### Environment Variables

| Variable | Default | Description |
|---|---|---|
| `OPENROUTER_API_KEY` | — | OpenRouter API key (or any OpenAI-compat key) |
| `ANTHROPIC_API_KEY` | — | Anthropic API key (auto-selects Anthropic transport) |
| `GARUDUST_MODEL` | `anthropic/claude-sonnet-4-6` | Model identifier |
| `GARUDUST_BASE_URL` | `https://openrouter.ai/api/v1` | Override API base URL |
| `GARUDUST_PORT` | `3000` | HTTP gateway port |
| `TELEGRAM_TOKEN` | — | Telegram bot token |
| `DISCORD_TOKEN` | — | Discord bot token |
| `RUST_LOG` | `warn` | Log level (`info`, `debug`, …) |

## Built-in Tools

| Tool | Toolset | Description |
|---|---|---|
| `web_fetch` | web | Fetch content from a URL |
| `read_file` | files | Read a file from the filesystem |
| `write_file` | files | Write content to a file |
| `terminal` | terminal | Run a shell command (with approval) |
| `memory` | memory | Add / read / replace / remove memory entries |
| `skills_list` | skills | List all available skills |
| `skill_view` | skills | Load a skill's full instructions by name |

## Skills

Place a `SKILL.md` file anywhere under `~/.garudust/skills/`:

```
~/.garudust/skills/
  my-skill/
    SKILL.md
  another-skill/
    SKILL.md
```

Minimal `SKILL.md` format:

```markdown
---
name: my-skill
description: Does something useful
version: 1.0.0
platforms: [macos, linux]   # omit to load on all platforms
---

# My Skill

Full instructions here…
```

The agent sees the skills index in every system prompt and can load individual skills with `skill_view`.

## Memory

The agent persists knowledge across sessions in two files:

- `~/.garudust/memories/MEMORY.md` — facts, conventions, tool quirks
- `~/.garudust/memories/USER.md` — user preferences and profile

Both are injected into the system prompt as a frozen snapshot at session start. The agent updates them mid-session via the `memory` tool; changes take effect next session.

## Adding a Platform

Implement the `PlatformAdapter` trait from `garudust-core`:

```rust
#[async_trait]
impl PlatformAdapter for MyPlatform {
    fn name(&self) -> &'static str { "myplatform" }
    async fn start(&self, handler: Arc<dyn MessageHandler>) -> Result<(), PlatformError> { … }
    async fn send_message(&self, channel: &ChannelId, message: OutboundMessage) -> Result<(), PlatformError> { … }
    async fn send_stream(&self, channel: &ChannelId, stream: Pin<Box<dyn Stream<Item = String> + Send>>) -> Result<(), PlatformError> { … }
}
```

Register it in `garudust-server/src/main.rs` alongside the existing Telegram/Discord adapters.

## Adding a Tool

Implement the `Tool` trait from `garudust-core`:

```rust
pub struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &'static str { "my_tool" }
    fn description(&self) -> &'static str { "Does something" }
    fn toolset(&self) -> &'static str { "mytoolset" }
    fn schema(&self) -> serde_json::Value { json!({ "type": "object", "properties": {} }) }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        Ok(ToolResult::ok("", "result"))
    }
}
```

Register it with `registry.register(MyTool)` before creating the agent.

## What's Next

- [ ] Streaming output (SSE in gateway + TUI delta rendering)
- [ ] Session persistence to SQLite
- [ ] `delegate_task` — spawn parallel subagents
- [ ] `session_search` — FTS5 search across past conversations
- [ ] `web_search` — Exa / SearXNG integration
- [ ] Bedrock and Codex transport adapters
- [ ] Browser tool (CDP via `chromiumoxide`)
- [ ] TTS / STT voice mode
- [ ] `/model`, `/new`, `/memory` slash commands in TUI
- [ ] Slack, Matrix, Signal platform adapters

## License

MIT
