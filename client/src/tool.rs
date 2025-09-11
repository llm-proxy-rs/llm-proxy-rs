use anyhow::Result;
use request::{Contents, Message, tool::Tool as RequestTool};

pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
}

impl From<ToolResult> for Message {
    fn from(result: ToolResult) -> Self {
        Message::Tool {
            contents: Contents::String(result.content),
            tool_call_id: result.tool_call_id,
        }
    }
}

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> RequestTool;
    async fn execute(&self, args: &str) -> Result<String>;
}
