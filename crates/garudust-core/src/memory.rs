use async_trait::async_trait;

use crate::error::AgentError;

pub const ENTRY_DELIMITER: &str = "\n§\n";

#[derive(Debug, Clone, Default)]
pub struct MemoryContent {
    pub entries: Vec<String>,
}

impl MemoryContent {
    pub fn parse(raw: &str) -> Self {
        let entries = raw
            .split(ENTRY_DELIMITER)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Self { entries }
    }

    pub fn serialize(&self) -> String {
        self.entries.join(ENTRY_DELIMITER)
    }
}

#[async_trait]
pub trait MemoryStore: Send + Sync + 'static {
    async fn read_memory(&self) -> Result<MemoryContent, AgentError>;
    async fn write_memory(&self, content: &MemoryContent) -> Result<(), AgentError>;

    async fn read_user_profile(&self) -> Result<String, AgentError>;
    async fn write_user_profile(&self, content: &str) -> Result<(), AgentError>;
}
