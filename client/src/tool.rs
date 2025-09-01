use anyhow::Result;
use request::tool::Tool as RequestTool;
use request::{Contents, Message};

/// Trait for all tools that can be executed
pub trait Tool {
    /// Get the tool definition for the API (now returns the proper request::Tool struct)
    fn definition(&self) -> RequestTool;

    /// Execute the tool with optional arguments
    fn execute(&self, args: Option<&str>) -> Result<String>;
}

pub struct ToolResult {
    pub id: String,
    pub name: String,
    pub result: String,
}

impl From<ToolResult> for Message {
    fn from(tool_result: ToolResult) -> Self {
        Message::Tool {
            contents: Contents::String(tool_result.result),
            tool_call_id: tool_result.id,
        }
    }
}
