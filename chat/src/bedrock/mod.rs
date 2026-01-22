use aws_sdk_bedrockruntime::types::{
    InferenceConfiguration, Message, SystemContentBlock, ToolConfiguration,
};
use aws_smithy_types::Document;
use serde::{Deserialize, Serialize};

pub mod anthropic;
pub mod openai;

pub use anthropic::*;
pub use openai::*;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct ReasoningEffortToThinkingBudgetTokens {
    pub low: i32,
    pub medium: i32,
    pub high: i32,
}

impl Default for ReasoningEffortToThinkingBudgetTokens {
    fn default() -> Self {
        Self {
            low: 1024,
            medium: 2048,
            high: 4096,
        }
    }
}

pub struct BedrockChatCompletion {
    pub model_id: String,
    pub messages: Option<Vec<Message>>,
    pub system_content_blocks: Option<Vec<SystemContentBlock>>,
    pub tool_config: Option<ToolConfiguration>,
    pub inference_config: InferenceConfiguration,
    pub additional_model_request_fields: Option<Document>,
}
