use anyhow::Result;
use aws_sdk_bedrockruntime::types::{Message, SystemContentBlock, ToolConfiguration};
use request::{ChatCompletionsRequest, Role};

pub struct BedrockChatCompletion {
    pub model_id: String,
    pub messages: Vec<Message>,
    pub system_content_blocks: Vec<SystemContentBlock>,
    pub tool_config: Option<ToolConfiguration>,
}

pub fn process_chat_completions_request_to_bedrock_chat_completion(
    request: &ChatCompletionsRequest,
) -> Result<BedrockChatCompletion> {
    let mut system_content_blocks = Vec::new();
    let mut messages = Vec::new();

    for request_message in &request.messages {
        match request_message.role {
            Role::Assistant | Role::Tool | Role::User => {
                messages.push(Message::try_from(request_message)?);
            }
            Role::System => {
                if let Some(contents) = &request_message.contents {
                    system_content_blocks.extend::<Vec<SystemContentBlock>>(contents.into());
                }
            }
        }
    }

    let tool_config = Option::<ToolConfiguration>::try_from(request)?;

    Ok(BedrockChatCompletion {
        model_id: request.model.clone(),
        messages,
        system_content_blocks,
        tool_config,
    })
}
