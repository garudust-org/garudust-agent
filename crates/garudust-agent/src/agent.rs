use std::sync::Arc;

use garudust_core::{
    budget::IterationBudget,
    config::AgentConfig,
    error::AgentError,
    memory::MemoryStore,
    tool::ToolContext,
    transport::ProviderTransport,
    types::{AgentResult, ContentPart, InferenceConfig, Message, Role, StopReason, ToolResult},
};
use garudust_tools::ToolRegistry;
use tracing::{debug, info};
use uuid::Uuid;

use crate::compressor::ContextCompressor;
use crate::prompt_builder::build_system_prompt;

pub struct Agent {
    id:         String,
    transport:  Arc<dyn ProviderTransport>,
    tools:      Arc<ToolRegistry>,
    memory:     Arc<dyn MemoryStore>,
    budget:     Arc<IterationBudget>,
    config:     Arc<AgentConfig>,
    compressor: ContextCompressor,
}

impl Agent {
    pub fn new(
        transport: Arc<dyn ProviderTransport>,
        tools:     Arc<ToolRegistry>,
        memory:    Arc<dyn MemoryStore>,
        config:    Arc<AgentConfig>,
    ) -> Self {
        let budget     = Arc::new(IterationBudget::new(config.max_iterations));
        let comp_model = config.compression.model
            .clone()
            .unwrap_or_else(|| config.model.clone());
        let compressor = ContextCompressor::new(transport.clone(), comp_model);
        Self { id: Uuid::new_v4().to_string(), transport, tools, memory, budget, config, compressor }
    }

    pub fn spawn_child(&self) -> Self {
        let comp_model = self.config.compression.model
            .clone()
            .unwrap_or_else(|| self.config.model.clone());
        Self {
            id:         Uuid::new_v4().to_string(),
            transport:  self.transport.clone(),
            tools:      self.tools.clone(),
            memory:     self.memory.clone(),
            budget:     self.budget.clone(),
            config:     self.config.clone(),
            compressor: ContextCompressor::new(self.transport.clone(), comp_model),
        }
    }

    pub async fn run(
        &self,
        task:     &str,
        approver: Arc<dyn garudust_core::tool::CommandApprover>,
        platform: &str,
    ) -> Result<AgentResult, AgentError> {
        let session_id    = Uuid::new_v4().to_string();
        let system_prompt = build_system_prompt(&self.config, self.memory.as_ref(), platform).await;
        let inf_config    = InferenceConfig {
            model:            self.config.model.clone(),
            max_tokens:       Some(8192),
            temperature:      None,
            reasoning_effort: None,
        };

        let mut history: Vec<Message> = vec![
            Message::system(&system_prompt),
            Message::user(task),
        ];

        let schemas      = self.tools.all_schemas();
        let mut total_in = 0u32;
        let mut total_out= 0u32;
        let mut iters    = 0u32;

        loop {
            // Compress if needed before every LLM call
            if self.config.compression.enabled && self.compressor.should_compress(&history) {
                info!("compressing context before turn {}", iters + 1);
                let (compressed, usage) = self.compressor.compress(history).await?;
                history    = compressed;
                total_in  += usage.input_tokens;
                total_out += usage.output_tokens;
            }

            self.budget.consume()?;
            iters += 1;
            info!(agent_id = %self.id, iteration = iters, "agent turn");

            let resp = self.transport.chat(&history, &inf_config, &schemas).await?;
            total_in  += resp.usage.input_tokens;
            total_out += resp.usage.output_tokens;

            history.push(Message {
                role:    Role::Assistant,
                content: resp.content.clone(),
            });

            if resp.tool_calls.is_empty() || resp.stop_reason == StopReason::EndTurn {
                let output = resp.content.iter()
                    .filter_map(|p| if let ContentPart::Text(t) = p { Some(t.as_str()) } else { None })
                    .collect::<Vec<_>>()
                    .join("\n");
                return Ok(AgentResult {
                    output,
                    usage: garudust_core::types::TokenUsage {
                        input_tokens:  total_in,
                        output_tokens: total_out,
                        ..Default::default()
                    },
                    iterations: iters,
                    session_id,
                });
            }

            // Parallel tool dispatch via tokio::join_all
            let ctx = Arc::new(ToolContext {
                session_id: session_id.clone(),
                agent_id:   self.id.clone(),
                iteration:  iters,
                budget:     self.budget.clone(),
                memory:     self.memory.clone(),
                config:     self.config.clone(),
                approver:   approver.clone(),
            });

            let tool_futs: Vec<_> = resp.tool_calls.iter().map(|tc| {
                let tools = self.tools.clone();
                let ctx   = ctx.clone();
                let name  = tc.name.clone();
                let args  = tc.arguments.clone();
                let id    = tc.id.clone();
                async move {
                    debug!(tool = %name, "dispatching");
                    let res = tools.dispatch(&name, args, &ctx).await;
                    let tr  = match res {
                        Ok(r)  => r,
                        Err(e) => ToolResult::err(&id, e.to_string()),
                    };
                    Message {
                        role: Role::Tool,
                        content: vec![ContentPart::ToolResult {
                            tool_use_id: id,
                            content:     tr.content,
                            is_error:    tr.is_error,
                        }],
                    }
                }
            }).collect();

            let tool_msgs = futures::future::join_all(tool_futs).await;
            history.extend(tool_msgs);
        }
    }
}
