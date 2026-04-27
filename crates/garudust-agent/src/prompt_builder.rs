use std::path::Path;

use garudust_core::config::AgentConfig;
use garudust_core::memory::MemoryStore;
use garudust_tools::toolsets::skills::build_skills_index;

pub async fn build_system_prompt(
    config: &AgentConfig,
    memory: &dyn MemoryStore,
    platform: &str,
) -> String {
    let mut parts = Vec::new();

    parts.push(AGENT_IDENTITY.to_string());

    // SOUL.md — persona file
    if let Ok(soul) = read_context_file(&config.home_dir.join("SOUL.md")).await {
        parts.push(format!("# Persona\n{soul}"));
    }

    // AGENTS.md — project context (walk up from cwd)
    if let Ok(agents) = read_context_file(Path::new("AGENTS.md")).await {
        parts.push(format!("# Project Context\n{agents}"));
    }

    // Skills index
    let skills_index = build_skills_index(&config.home_dir.join("skills"), platform).await;
    if !skills_index.is_empty() {
        parts.push(skills_index);
    }

    // Memory
    if let Ok(mem) = memory.read_memory().await {
        let content = mem.serialize_for_prompt();
        if !content.is_empty() {
            parts.push(format!("# Memory\n{content}"));
        }
    }

    // User profile
    if let Ok(profile) = memory.read_user_profile().await {
        if !profile.is_empty() {
            parts.push(format!("# User Profile\n{profile}"));
        }
    }

    parts.join("\n\n---\n\n")
}

async fn read_context_file(path: &Path) -> std::io::Result<String> {
    tokio::fs::read_to_string(path).await
}

const AGENT_IDENTITY: &str = "\
You are Garudust, a powerful self-improving AI agent. You have access to tools \
to help complete tasks. Think step by step, use tools when needed, and always \
provide clear, accurate responses. When you finish a complex task, distill what \
you learned into memory using the `memory` tool.";
