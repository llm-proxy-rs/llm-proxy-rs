use anthropic_request::{V1MessagesRequest, build_tool_configuration};
use anyhow::Result;
use aws_sdk_bedrockruntime::types::{
    ContentBlock, InferenceConfiguration, Message as BedrockMessage,
    OutputConfig as BedrockOutputConfig, SystemContentBlock,
};

use crate::bedrock::BedrockChatCompletion;

/// Bedrock returns "The toolConfig field must be defined when using toolUse and toolResult
/// content blocks." when no tool configuration is present. Remove them so prior tool-augmented
/// conversation history can be forwarded without a tools field.
pub fn strip_tool_blocks(messages: Vec<BedrockMessage>) -> Result<Vec<BedrockMessage>> {
    messages
        .into_iter()
        .filter_map(|msg| {
            let content: Vec<ContentBlock> = msg
                .content()
                .iter()
                .filter(|block| {
                    !matches!(
                        block,
                        ContentBlock::ToolUse(_) | ContentBlock::ToolResult(_)
                    )
                })
                .cloned()
                .collect();
            if content.is_empty() {
                None
            } else {
                Some(
                    BedrockMessage::builder()
                        .role(msg.role().clone())
                        .set_content(Some(content))
                        .build()
                        .map_err(anyhow::Error::from),
                )
            }
        })
        .collect()
}

impl TryFrom<&V1MessagesRequest> for BedrockChatCompletion {
    type Error = anyhow::Error;

    fn try_from(request: &V1MessagesRequest) -> Result<Self, Self::Error> {
        let messages = request.messages.to_bedrock_messages()?;

        let system_content_blocks = request
            .system
            .as_ref()
            .map(Vec::<SystemContentBlock>::try_from)
            .transpose()?;

        let tool_config = request
            .tools
            .as_deref()
            .map(|tools| build_tool_configuration(tools, request.tool_choice.as_ref()))
            .transpose()?
            .flatten();

        let messages = if tool_config.is_none() {
            messages.map(strip_tool_blocks).transpose()?
        } else {
            messages
        };

        let inference_config = InferenceConfiguration::builder()
            .max_tokens(request.max_tokens)
            .set_temperature(request.temperature)
            .set_top_p(request.top_p)
            .set_stop_sequences(request.stop_sequences.clone())
            .build();

        let output_config = request
            .output_config
            .as_ref()
            .map(Option::<BedrockOutputConfig>::try_from)
            .transpose()?
            .flatten();

        Ok(BedrockChatCompletion {
            model_id: request.model.clone(),
            messages,
            system_content_blocks,
            tool_config,
            inference_config,
            output_config,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_bedrockruntime::types::ConversationRole;

    fn base_request(extra: serde_json::Value) -> V1MessagesRequest {
        let mut json = serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hi"}]
        });
        if let (Some(base), serde_json::Value::Object(extra)) = (json.as_object_mut(), extra) {
            base.extend(extra);
        }
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn tool_choice_none_produces_no_tool_choice_on_bedrock() {
        let request = base_request(serde_json::json!({
            "tools": [{"name": "get_weather", "input_schema": {"type": "object"}}],
            "tool_choice": {"type": "none"}
        }));
        let result = BedrockChatCompletion::try_from(&request).unwrap();
        let tool_config = result.tool_config.unwrap();
        assert!(tool_config.tool_choice().is_none());
    }

    #[test]
    fn no_tool_choice_field_produces_no_tool_choice_on_bedrock() {
        let request = base_request(serde_json::json!({
            "tools": [{"name": "get_weather", "input_schema": {"type": "object"}}]
        }));
        let result = BedrockChatCompletion::try_from(&request).unwrap();
        let tool_config = result.tool_config.unwrap();
        assert!(tool_config.tool_choice().is_none());
    }

    #[test]
    fn tool_blocks_stripped_when_no_tools_field() {
        let request = base_request(serde_json::json!({
            "messages": [
                {"role": "user", "content": "What's the weather?"},
                {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "tool_1", "name": "get_weather", "input": {"city": "NYC"}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "tool_1", "content": "Sunny"}
                ]}
            ]
        }));
        let result = BedrockChatCompletion::try_from(&request).unwrap();
        assert!(result.tool_config.is_none());
        // Messages with only tool blocks should be removed entirely
        let msgs = result.messages.unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].role(), &ConversationRole::User);
    }

    #[test]
    fn no_tool_config_when_no_tool_blocks_and_no_tools_field() {
        let request = base_request(serde_json::json!({
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        }));
        let result = BedrockChatCompletion::try_from(&request).unwrap();
        assert!(result.tool_config.is_none());
    }

    #[test]
    fn tool_config_preserved_when_tools_and_tool_blocks_both_present() {
        let request = base_request(serde_json::json!({
            "tools": [{"name": "get_weather", "input_schema": {"type": "object"}}],
            "messages": [
                {"role": "user", "content": "What's the weather?"},
                {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "tool_1", "name": "get_weather", "input": {"city": "NYC"}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "tool_1", "content": "Sunny"}
                ]}
            ]
        }));
        let result = BedrockChatCompletion::try_from(&request).unwrap();
        let tool_config = result.tool_config.unwrap();
        assert!(!tool_config.tools().is_empty());
    }
}
