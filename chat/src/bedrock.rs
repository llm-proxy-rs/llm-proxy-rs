use anyhow::Result;
use aws_sdk_bedrockruntime::types::{Message, SystemContentBlock, ToolConfiguration};
use aws_smithy_types::Document;
use request::{ChatCompletionsRequest, Role};

const THINKING_BUDGET_TOKENS: i32 = 4096;

pub struct BedrockChatCompletion {
    pub model_id: String,
    pub messages: Vec<Message>,
    pub system_content_blocks: Vec<SystemContentBlock>,
    pub tool_config: Option<ToolConfiguration>,
    pub additional_model_request_fields: Option<Document>,
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

    let additional_model_request_fields = request.reasoning_effort.as_ref().map(|_| {
        Document::Object(
            [(
                "thinking".to_string(),
                Document::Object(
                    [
                        ("type".to_string(), Document::String("enabled".to_string())),
                        (
                            "budget_tokens".to_string(),
                            Document::from(THINKING_BUDGET_TOKENS),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
            )]
            .into_iter()
            .collect(),
        )
    });

    Ok(BedrockChatCompletion {
        model_id: request.model.clone(),
        messages,
        system_content_blocks,
        tool_config,
        additional_model_request_fields,
    })
}
