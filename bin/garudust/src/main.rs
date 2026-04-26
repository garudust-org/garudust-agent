mod tui;

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use garudust_agent::{Agent, AutoApprover};
use garudust_core::config::AgentConfig;
use garudust_memory::FileMemoryStore;
use garudust_tools::{
    ToolRegistry,
    toolsets::{
        files::{ReadFile, WriteFile},
        memory::MemoryTool,
        skills::{SkillsList, SkillView},
        terminal::Terminal,
        web::WebFetch,
    },
};
use garudust_transport::build_transport;
use tokio::sync::mpsc;

use tui::{AgentEvent, TuiEvent};

#[derive(Parser)]
#[command(name = "garudust", about = "Garudust AI Agent")]
struct Cli {
    /// One-shot task (omit to start interactive TUI)
    task: Option<String>,

    #[arg(long, env = "GARUDUST_MODEL", default_value = "anthropic/claude-sonnet-4-6")]
    model: String,

    #[arg(long, env = "OPENROUTER_API_KEY")]
    api_key: Option<String>,

    #[arg(long, env = "ANTHROPIC_API_KEY")]
    anthropic_key: Option<String>,

    #[arg(long, env = "GARUDUST_BASE_URL")]
    base_url: Option<String>,
}

fn build_agent(cli: &Cli) -> Arc<Agent> {
    let mut config  = AgentConfig::default();
    config.model    = cli.model.clone();
    config.base_url = cli.base_url.clone();

    // Prefer Anthropic key if base_url points to anthropic, else OpenRouter
    if let Some(k) = &cli.anthropic_key {
        config.api_key  = Some(k.clone());
        config.provider = "anthropic".into();
    } else {
        config.api_key = cli.api_key.clone();
    }

    let config    = Arc::new(config);
    let memory    = Arc::new(FileMemoryStore::new(&config.home_dir));
    let transport = build_transport(&config);

    let mut registry = ToolRegistry::new();
    registry.register(WebFetch);
    registry.register(ReadFile);
    registry.register(WriteFile);
    registry.register(Terminal);
    registry.register(MemoryTool);
    registry.register(SkillsList);
    registry.register(SkillView);

    Arc::new(Agent::new(transport, Arc::new(registry), memory, config))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".into()))
        .init();
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    if let Some(task) = &cli.task {
        // ── One-shot mode ────────────────────────────────────────────────
        let agent    = build_agent(&cli);
        let approver = Arc::new(AutoApprover);
        let result   = agent.run(task, approver, "cli").await?;
        println!("{}", result.output);
        eprintln!("[{} iter | {}in {}out tokens]",
            result.iterations, result.usage.input_tokens, result.usage.output_tokens);
    } else {
        // ── Interactive TUI mode ─────────────────────────────────────────
        let agent    = build_agent(&cli);
        let approver = Arc::new(AutoApprover);

        let (tx_event, mut rx_event) = mpsc::channel::<TuiEvent>(32);
        let (tx_agent, rx_agent)     = mpsc::channel::<AgentEvent>(64);

        // Agent task — processes Submit events
        let agent2    = agent.clone();
        let approver2 = approver.clone();
        let tx_agent2 = tx_agent.clone();
        tokio::spawn(async move {
            while let Some(ev) = rx_event.recv().await {
                match ev {
                    TuiEvent::Quit => break,
                    TuiEvent::Submit(task) => {
                        let _ = tx_agent2.send(AgentEvent::Thinking).await;
                        match agent2.run(&task, approver2.clone(), "cli").await {
                            Ok(r) => {
                                let _ = tx_agent2.send(AgentEvent::Output(r.output)).await;
                                let _ = tx_agent2.send(AgentEvent::Done {
                                    iterations:    r.iterations,
                                    input_tokens:  r.usage.input_tokens,
                                    output_tokens: r.usage.output_tokens,
                                }).await;
                            }
                            Err(e) => {
                                let _ = tx_agent2.send(AgentEvent::Error(e.to_string())).await;
                            }
                        }
                    }
                }
            }
        });

        tui::Tui::run(tx_event, rx_agent).await?;
    }

    Ok(())
}
