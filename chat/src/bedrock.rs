use anyhow::Result;
use aws_sdk_bedrockruntime::types::{
    InferenceConfiguration, Message, SystemContentBlock, ToolConfiguration,
};
use aws_smithy_types::Document;
use request::ChatCompletionsRequest;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReasoningEffortToThinkingBudgetTokens {
    pub low: i32,
    pub medium: i32,
    pub high: i32,
}

impl Default for ReasoningEffortToThinkingBudgetTokens {
    fn default() -> Self {
        Self {
            low: 1024,
            medium: 2048,
            high: 4096,
        }
    }
}

pub struct BedrockChatCompletion {
    pub model_id: String,
    pub messages: Vec<Message>,
    pub system_content_blocks: Vec<SystemContentBlock>,
    pub tool_config: Option<ToolConfiguration>,
    pub inference_config: InferenceConfiguration,
    pub additional_model_request_fields: Option<Document>,
}

pub fn process_chat_completions_request_to_bedrock_chat_completion(
    request: &ChatCompletionsRequest,
    reasoning_effort_to_thinking_budget_tokens: &ReasoningEffortToThinkingBudgetTokens,
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

    let additional_model_request_fields = request.reasoning_effort.as_ref().map(|effort| {
        let budget_tokens = match effort.to_lowercase().as_str() {
            "low" => reasoning_effort_to_thinking_budget_tokens.low,
            "medium" => reasoning_effort_to_thinking_budget_tokens.medium,
            "high" => reasoning_effort_to_thinking_budget_tokens.high,
            _ => reasoning_effort_to_thinking_budget_tokens.low,
        };

        Document::Object(
            [(
                "thinking".to_string(),
                Document::Object(
                    [
                        ("type".to_string(), Document::String("enabled".to_string())),
                        ("budget_tokens".to_string(), Document::from(budget_tokens)),
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
        inference_config,
        additional_model_request_fields,
    })
}
