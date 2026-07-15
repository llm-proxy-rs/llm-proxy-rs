use aws_sdk_bedrockruntime::types::{
    InferenceConfiguration, Message, OutputConfig, SystemContentBlock, ToolConfiguration,
};

pub mod converse;
pub mod mantle;

pub struct BedrockChatCompletion {
    pub model_id: String,
    pub messages: Option<Vec<Message>>,
    pub system_content_blocks: Option<Vec<SystemContentBlock>>,
    pub tool_config: Option<ToolConfiguration>,
    pub inference_config: InferenceConfiguration,
    pub output_config: Option<OutputConfig>,
}
