use anthropic_request::{V1MessagesRequest, build_tool_configuration};
use anyhow::Result;
use aws_sdk_bedrockruntime::types::{
    InferenceConfiguration, OutputConfig as BedrockOutputConfig, SystemContentBlock,
};

use crate::bedrock::BedrockChatCompletion;

impl TryFrom<&V1MessagesRequest> for BedrockChatCompletion {
    type Error = anyhow::Error;

    fn try_from(request: &V1MessagesRequest) -> Result<Self, Self::Error> {
        let messages = Option::<Vec<_>>::try_from(&request.messages)?;

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
}
