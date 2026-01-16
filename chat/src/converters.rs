use anthropic_request::V1MessagesRequest;
use anyhow::Result;
use aws_sdk_bedrockruntime::types::{
    InferenceConfiguration, Message as BedrockMessage, SystemContentBlock, Tool as BedrockTool,
    ToolChoice as BedrockToolChoice, ToolConfiguration,
};
use aws_smithy_types::Document;

use crate::bedrock::BedrockChatCompletion;

impl TryFrom<&V1MessagesRequest> for BedrockChatCompletion {
    type Error = anyhow::Error;

    fn try_from(request: &V1MessagesRequest) -> Result<Self, Self::Error> {
        let system_content_blocks = if let Some(ref system) = request.system {
            Vec::<SystemContentBlock>::from(system)
        } else {
            Vec::new()
        };

        let messages = request
            .messages
            .iter()
            .map(BedrockMessage::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        let tool_config = if request.tools.as_ref().is_some_and(|t| !t.is_empty()) {
            let mut builder = ToolConfiguration::builder();

            if let Some(tools) = &request.tools {
                for tool in tools {
                    let bedrock_tool = BedrockTool::try_from(tool)?;
                    builder = builder.tools(bedrock_tool);
                }
            }

            if let Some(tool_choice) = &request.tool_choice {
                let bedrock_tool_choice = Option::<BedrockToolChoice>::try_from(tool_choice)?;
                builder = builder.set_tool_choice(bedrock_tool_choice);
            }

            Some(builder.build()?)
        } else {
            None
        };

        let inference_config = InferenceConfiguration::builder()
            .set_max_tokens(Some(request.max_tokens))
            .set_temperature(request.temperature)
            .set_top_p(request.top_p)
            .build();

        // Handle extended thinking configuration
        let additional_model_request_fields = if let Some(thinking) = &request.thinking {
            let mut thinking_obj = vec![(
                "type".to_string(),
                Document::String(thinking.thinking_type.clone()),
            )];

            if let Some(budget) = thinking.budget_tokens {
                thinking_obj.push(("budget_tokens".to_string(), Document::from(budget)));
            }

            Some(Document::Object(
                [(
                    "thinking".to_string(),
                    Document::Object(thinking_obj.into_iter().collect()),
                )]
                .into_iter()
                .collect(),
            ))
        } else {
            None
        };

        Ok(BedrockChatCompletion {
            model_id: request.model.clone(),
            messages,
            system_content_blocks,
            tool_config,
            inference_config,
            additional_model_request_fields,
        })
    }
}
