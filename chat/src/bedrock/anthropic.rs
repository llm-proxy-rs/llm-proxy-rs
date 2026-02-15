use anthropic_request::{V1MessagesRequest, additional_model_request_fields};
use anyhow::Result;
use aws_sdk_bedrockruntime::types::{
    InferenceConfiguration, OutputConfig as BedrockOutputConfig, SystemContentBlock,
};

use super::BedrockChatCompletion;

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
            .map(anthropic_request::tools_to_tool_configuration)
            .transpose()?
            .flatten();

        let inference_config = InferenceConfiguration::builder()
            .max_tokens(request.max_tokens)
            .set_temperature(request.temperature)
            .build();

        let output_config = request
            .output_config
            .as_ref()
            .map(Option::<BedrockOutputConfig>::try_from)
            .transpose()?
            .flatten();

        let additional_model_request_fields = additional_model_request_fields(
            request.thinking.as_ref(),
            request.output_config.as_ref(),
        );

        Ok(BedrockChatCompletion {
            model_id: request.model.clone(),
            messages,
            system_content_blocks,
            tool_config,
            inference_config,
            additional_model_request_fields,
            output_config,
        })
    }
}
