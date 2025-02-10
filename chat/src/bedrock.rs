use aws_sdk_bedrockruntime::types::{Message, SystemContentBlock};
use bedrock::content_blocks::ToBedrockContentBlocks;
use bedrock::message::ToBedrockMessage;
use request::{ChatCompletionsRequest, Role};

use crate::ProcessChatCompletionsRequest;

pub struct BedrockChatCompletion {
    pub model_id: String,
    pub system_content_blocks: Vec<SystemContentBlock>,
    pub messages: Vec<Message>,
}

impl ProcessChatCompletionsRequest<BedrockChatCompletion> for ChatCompletionsRequest {
    fn process_chat_completions_request(&self) -> BedrockChatCompletion {
        let mut system_content_blocks = Vec::new();
        let mut messages = Vec::new();
        let model_id = self.model.clone();

        for request_message in &self.messages {
            match request_message.role {
                Role::Assistant | Role::User => {
                    if let Some(message) = request_message.to_bedrock_message() {
                        messages.push(message);
                    }
                }
                Role::System => {
                    system_content_blocks
                        .extend(request_message.contents.to_bedrock_content_blocks());
                }
            }
        }

        BedrockChatCompletion {
            model_id,
            system_content_blocks,
            messages,
        }
    }
}
