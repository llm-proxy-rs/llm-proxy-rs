use aws_sdk_bedrockruntime::types::{Message, SystemContentBlock};
use bedrock::{
    content_blocks::contents_to_bedrock_system_content_block,
    message::request_message_to_bedrock_message,
};
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
                    if let Some(message) = request_message_to_bedrock_message(request_message) {
                        messages.push(message);
                    }
                }
                Role::System => {
                    system_content_blocks.extend(contents_to_bedrock_system_content_block(
                        &request_message.contents,
                    ));
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
