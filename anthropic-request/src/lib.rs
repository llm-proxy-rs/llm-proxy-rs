use serde::{Deserialize, Serialize};

pub mod additional_model_request_fields;
pub mod cache_control;
pub mod content;
pub mod document_source;
pub mod image_source;
pub mod message;
pub mod output_config;
pub mod system;
pub mod thinking;
pub mod tool;
pub mod tool_result_content;

pub use additional_model_request_fields::*;
pub use cache_control::*;
pub use content::*;
pub use document_source::*;
pub use image_source::*;
pub use message::*;
pub use output_config::*;
pub use system::*;
pub use thinking::*;
pub use tool::*;
pub use tool_result_content::*;

#[derive(Debug, Deserialize, Serialize)]
pub struct V1MessagesRequest {
    pub max_tokens: i32,
    pub messages: Messages,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Systems>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<Thinking>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_config: Option<OutputConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct V1MessagesCountTokensRequest {
    pub messages: Messages,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Systems>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<Thinking>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
}
