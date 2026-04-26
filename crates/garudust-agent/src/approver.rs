use async_trait::async_trait;
use garudust_core::tool::{ApprovalDecision, CommandApprover};

/// Auto-approves every command — used in non-interactive / server mode.
pub struct AutoApprover;

#[async_trait]
impl CommandApprover for AutoApprover {
    async fn approve(&self, _command: &str, _description: &str) -> ApprovalDecision {
        ApprovalDecision::Approved
    }
}

/// Always denies — useful for read-only agents.
pub struct DenyApprover;

#[async_trait]
impl CommandApprover for DenyApprover {
    async fn approve(&self, _command: &str, _description: &str) -> ApprovalDecision {
        ApprovalDecision::Denied
    }
}
