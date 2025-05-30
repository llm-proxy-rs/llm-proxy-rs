use aws_sdk_bedrockruntime::types::{Message, SystemContentBlock};
use bedrock::message::request_message_to_bedrock_message;
use request::{ChatCompletionsRequest, Role};

pub struct BedrockChatCompletion {
    pub model_id: String,
    pub system_content_blocks: Vec<SystemContentBlock>,
    pub messages: Vec<Message>,
}

pub fn process_chat_completions_request_to_bedrock_chat_completion(
    request: &ChatCompletionsRequest,
) -> BedrockChatCompletion {
    let mut system_content_blocks = Vec::new();
    let mut messages = Vec::new();
    let model_id = request.model.clone();

    for request_message in &request.messages {
        match request_message.role {
            Role::Assistant | Role::User => {
                if let Some(message) = request_message_to_bedrock_message(request_message) {
                    messages.push(message);
                }
            }
            Role::System => {
                let system_blocks: Vec<SystemContentBlock> = (&request_message.contents).into();
                system_content_blocks.extend(system_blocks);
            }
        }
    }

    BedrockChatCompletion {
        model_id,
        system_content_blocks,
        messages,
    }
}
