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

/// Blocks commands that match known-dangerous patterns; approves everything else.
///
/// Substring matching is intentionally simple — it is a defense-in-depth layer,
/// not a sandbox. Obfuscated commands (variable expansion, eval) can bypass it.
pub struct SmartApprover;

static DANGEROUS_PATTERNS: &[&str] = &[
    "rm -rf",
    "rm -fr",
    "> /dev/sd",
    "mkfs",
    "dd if=",
    "drop table",
    "drop database",
    "truncate table",
    "curl | sh",
    "curl|sh",
    "wget | sh",
    "wget|sh",
    "curl | bash",
    "curl|bash",
    "wget | bash",
    "wget|bash",
    "; rm ",
    "&& rm ",
    "| rm ",
    "chmod 777",
    "chown root",
    "sudo rm",
    "sudo dd",
    "> /etc/",
    ">> /etc/",
    "/etc/passwd",
    "/etc/shadow",
    "base64 -d |",
    "base64 -d|",
    "eval $(curl",
    "eval $(wget",
    "eval \"$(curl",
    "eval \"$(wget",
];

#[async_trait]
impl CommandApprover for SmartApprover {
    async fn approve(&self, command: &str, _description: &str) -> ApprovalDecision {
        let lower = command.to_lowercase();
        for pattern in DANGEROUS_PATTERNS {
            if lower.contains(pattern) {
                tracing::warn!(
                    command = %command,
                    pattern = %pattern,
                    "SmartApprover: blocked dangerous command"
                );
                return ApprovalDecision::Denied;
            }
        }
        ApprovalDecision::Approved
    }
}
