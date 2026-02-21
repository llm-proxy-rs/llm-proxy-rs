use anyhow::Result;
use aws_sdk_bedrockruntime::types::{
    InferenceConfiguration, Message, SystemContentBlock, ToolConfiguration,
};
use request::ChatCompletionsRequest;

use crate::bedrock::BedrockChatCompletion;

pub fn process_chat_completions_request_to_bedrock_chat_completion(
    request: &ChatCompletionsRequest,
) -> Result<BedrockChatCompletion> {
    let mut system_content_blocks = Vec::new();
    let mut messages = Vec::new();

    let mut message_iter = request.messages.iter().peekable();

    while let Some(request_message) = message_iter.next() {
        match request_message {
            request::Message::Assistant { .. } => {
                messages.push(Message::try_from(request_message)?);
            }
            request::Message::User { .. } => {
                messages.push(Message::try_from(request_message)?);
            }
            request::Message::Tool { .. } => {
                let mut tool_messages = vec![request_message];

                while let Some(next_message) = message_iter.peek() {
                    if let request::Message::Tool { .. } = next_message {
                        if let Some(tool_message) = message_iter.next() {
                            tool_messages.push(tool_message);
                        }
                    } else {
                        break;
                    }
                }

                let bedrock_message = request::tool_messages_to_bedrock_message(&tool_messages)?;
                messages.push(bedrock_message);
            }
            request::Message::System { contents } => {
                if let Some(contents) = contents {
                    system_content_blocks.extend::<Vec<SystemContentBlock>>(contents.into());
                }
            }
        }
    }

    let tool_config = Option::<ToolConfiguration>::try_from(request)?;

    let inference_config = InferenceConfiguration::builder()
        .set_max_tokens(request.max_tokens)
        .set_temperature(request.temperature)
        .set_top_p(request.top_p)
        .build();

    Ok(BedrockChatCompletion {
        model_id: request.model.clone(),
        messages: if messages.is_empty() {
            None
        } else {
            Some(messages)
        },
        system_content_blocks: if system_content_blocks.is_empty() {
            None
        } else {
            Some(system_content_blocks)
        },
        tool_config,
        inference_config,
        output_config: None,
    })
}
