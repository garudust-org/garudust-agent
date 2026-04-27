use async_trait::async_trait;
use chrono::Utc;

use crate::error::AgentError;

pub const ENTRY_DELIMITER: &str = "\n§\n";

#[derive(Debug, Clone, PartialEq, Default)]
pub enum MemoryCategory {
    Fact,
    Preference,
    Skill,
    Project,
    #[default]
    Other,
}

impl MemoryCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Preference => "preference",
            Self::Skill => "skill",
            Self::Project => "project",
            Self::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "fact" => Self::Fact,
            "preference" => Self::Preference,
            "skill" => Self::Skill,
            "project" => Self::Project,
            _ => Self::Other,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Fact => "Facts",
            Self::Preference => "Preferences",
            Self::Skill => "Skills",
            Self::Project => "Project",
            Self::Other => "Other",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub category: MemoryCategory,
    pub content: String,
    pub created_at: String, // YYYY-MM-DD
}

impl MemoryEntry {
    pub fn new(category: MemoryCategory, content: String) -> Self {
        let created_at = Utc::now().format("%Y-%m-%d").to_string();
        Self {
            category,
            content,
            created_at,
        }
    }

    /// Parse `[category|date] content` or plain `content` (backward compat → Other).
    fn parse(raw: &str) -> Self {
        let raw = raw.trim();
        if let Some(rest) = raw.strip_prefix('[') {
            if let Some(bracket_end) = rest.find(']') {
                let meta = &rest[..bracket_end];
                let content = rest[bracket_end + 1..].trim().to_string();
                let mut parts = meta.splitn(2, '|');
                let cat_str = parts.next().unwrap_or("other");
                let date = parts.next().unwrap_or("").to_string();
                return Self {
                    category: MemoryCategory::from_str(cat_str),
                    content,
                    created_at: date,
                };
            }
        }
        Self {
            category: MemoryCategory::Other,
            content: raw.to_string(),
            created_at: String::new(),
        }
    }

    fn serialize(&self) -> String {
        format!(
            "[{}|{}] {}",
            self.category.as_str(),
            self.created_at,
            self.content
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct MemoryContent {
    pub entries: Vec<MemoryEntry>,
}

impl MemoryContent {
    pub fn parse(raw: &str) -> Self {
        let entries = raw
            .split(ENTRY_DELIMITER)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| MemoryEntry::parse(s))
            .collect();
        Self { entries }
    }

    pub fn serialize(&self) -> String {
        self.entries
            .iter()
            .map(MemoryEntry::serialize)
            .collect::<Vec<_>>()
            .join(ENTRY_DELIMITER)
    }

    /// Grouped markdown for the system prompt.
    pub fn serialize_for_prompt(&self) -> String {
        if self.entries.is_empty() {
            return String::new();
        }

        let order = [
            MemoryCategory::Fact,
            MemoryCategory::Preference,
            MemoryCategory::Skill,
            MemoryCategory::Project,
            MemoryCategory::Other,
        ];

        let mut sections = Vec::new();
        for cat in &order {
            let items: Vec<&MemoryEntry> =
                self.entries.iter().filter(|e| &e.category == cat).collect();
            if items.is_empty() {
                continue;
            }
            let lines: Vec<String> = items
                .iter()
                .map(|e| {
                    if e.created_at.is_empty() {
                        format!("- {}", e.content)
                    } else {
                        format!("- {} ({})", e.content, e.created_at)
                    }
                })
                .collect();
            sections.push(format!("## {}\n{}", cat.display_name(), lines.join("\n")));
        }

        sections.join("\n\n")
    }
}

#[async_trait]
pub trait MemoryStore: Send + Sync + 'static {
    async fn read_memory(&self) -> Result<MemoryContent, AgentError>;
    async fn write_memory(&self, content: &MemoryContent) -> Result<(), AgentError>;

    async fn read_user_profile(&self) -> Result<String, AgentError>;
    async fn write_user_profile(&self, content: &str) -> Result<(), AgentError>;
}
