use async_trait::async_trait;
use garudust_core::{error::ToolError, tool::{Tool, ToolContext}, types::ToolResult};
use serde_json::json;

pub struct WebFetch;

#[async_trait]
impl Tool for WebFetch {
    fn name(&self) -> &'static str { "web_fetch" }
    fn description(&self) -> &'static str { "Fetch content from a URL" }
    fn toolset(&self) -> &'static str { "web" }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "URL to fetch" }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let url = params["url"].as_str()
            .ok_or_else(|| ToolError::InvalidArgs("url required".into()))?;

        let body = reqwest::get(url).await
            .map_err(|e| ToolError::Execution(e.to_string()))?
            .text().await
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        Ok(ToolResult::ok("", body))
    }
}
