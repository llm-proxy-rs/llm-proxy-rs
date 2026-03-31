use anyhow::Result;
use aws_sdk_bedrockruntime::types::{
    InferenceConfiguration, Message, SystemContentBlock, ToolConfiguration,
};
use request::ChatCompletionsRequest;

use crate::bedrock::BedrockChatCompletion;
use crate::bedrock::anthropic::strip_tool_blocks;

pub fn build_bedrock_chat_completion(
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

    let messages = if tool_config.is_none() {
        strip_tool_blocks(messages)?
    } else {
        messages
    };

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

#[cfg(test)]
mod tests {
    use super::*;

    fn base_request(extra: serde_json::Value) -> ChatCompletionsRequest {
        let mut json = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}]
        });
        if let (Some(base), serde_json::Value::Object(extra)) = (json.as_object_mut(), extra) {
            base.extend(extra);
        }
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn tool_blocks_stripped_when_no_tools_field() {
        let request = base_request(serde_json::json!({
            "messages": [
                {"role": "user", "content": "What's the weather?"},
                {"role": "assistant", "content": null, "tool_calls": [
                    {"id": "call_1", "type": "function", "function": {"name": "get_weather", "arguments": "{\"city\":\"NYC\"}"}}
                ]},
                {"role": "tool", "tool_call_id": "call_1", "content": "Sunny"}
            ]
        }));
        let result = build_bedrock_chat_completion(&request).unwrap();
        assert!(result.tool_config.is_none());
        let msgs = result.messages.unwrap();
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn no_tool_config_when_no_tool_blocks_and_no_tools_field() {
        let request = base_request(serde_json::json!({
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        }));
        let result = build_bedrock_chat_completion(&request).unwrap();
        assert!(result.tool_config.is_none());
    }

    #[test]
    fn tool_config_preserved_when_tools_and_tool_blocks_both_present() {
        let request = base_request(serde_json::json!({
            "tools": [{"type": "function", "function": {"name": "get_weather", "parameters": {"type": "object"}}}],
            "messages": [
                {"role": "user", "content": "What's the weather?"},
                {"role": "assistant", "content": null, "tool_calls": [
                    {"id": "call_1", "type": "function", "function": {"name": "get_weather", "arguments": "{\"city\":\"NYC\"}"}}
                ]},
                {"role": "tool", "tool_call_id": "call_1", "content": "Sunny"}
            ]
        }));
        let result = build_bedrock_chat_completion(&request).unwrap();
        let tool_config = result.tool_config.unwrap();
        assert!(!tool_config.tools().is_empty());
    }
}
