use serde::{Deserialize, Serialize};

pub mod delta;
pub mod events;

pub use delta::*;
pub use events::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnthropicResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String, // "message"
    pub role: String, // "assistant"
    pub content: Vec<ResponseContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum ResponseContentBlock {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    #[serde(rename = "thinking")]
    Thinking { thinking: String },
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Usage {
    pub input_tokens: i32,
    pub output_tokens: i32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<i32>,
}
