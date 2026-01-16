use aws_sdk_bedrockruntime::types::{
    ContentBlockDelta, ContentBlockStart, ConversationRole, ConverseStreamOutput,
    ReasoningContentBlockDelta, StopReason, ToolUseBlockDelta, ToolUseBlockStart,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize, Serialize)]
pub struct ChatCompletionsResponse {
    pub choices: Vec<Choice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Choice {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<Delta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    pub index: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Delta {
    Content { content: String },
    Role { role: String },
    ToolCalls { tool_calls: Vec<ToolCall> },
    Reasoning { reasoning_content: String },
    Empty {},
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub tool_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<Function>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Function {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Usage {
    pub completion_tokens: i32,
    pub prompt_tokens: i32,
    pub total_tokens: i32,
}

impl ChatCompletionsResponse {
    pub fn builder() -> ChatCompletionsResponseBuilder {
        ChatCompletionsResponseBuilder::default()
    }
}

#[derive(Default)]
pub struct ChatCompletionsResponseBuilder {
    choices: Vec<Choice>,
    created: Option<i64>,
    id: Option<String>,
    model: Option<String>,
    object: Option<String>,
    usage: Option<Usage>,
}

impl ChatCompletionsResponseBuilder {
    pub fn choice(mut self, choice: Choice) -> Self {
        self.choices.push(choice);
        self
    }

    pub fn created(mut self, created: Option<i64>) -> Self {
        self.created = created;
        self
    }

    pub fn id(mut self, id: Option<String>) -> Self {
        self.id = id;
        self
    }

    pub fn model(mut self, model: Option<String>) -> Self {
        self.model = model;
        self
    }

    pub fn object(mut self, object: Option<String>) -> Self {
        self.object = object;
        self
    }

    pub fn usage(mut self, usage: Option<Usage>) -> Self {
        self.usage = usage;
        self
    }

    pub fn build(self) -> ChatCompletionsResponse {
        ChatCompletionsResponse {
            choices: self.choices,
            created: self.created,
            id: self.id,
            model: self.model,
            object: self.object,
            usage: self.usage,
        }
    }
}

#[derive(Default)]
pub struct ChoiceBuilder {
    pub delta: Option<Delta>,
    pub finish_reason: Option<String>,
    pub index: i32,
    pub logprobs: Option<String>,
}

impl ChoiceBuilder {
    pub fn delta(mut self, delta: Option<Delta>) -> Self {
        self.delta = delta;
        self
    }

    pub fn finish_reason(mut self, reason: Option<String>) -> Self {
        self.finish_reason = reason;
        self
    }

    pub fn index(mut self, index: i32) -> Self {
        self.index = index;
        self
    }

    pub fn logprobs(mut self, logprobs: Option<String>) -> Self {
        self.logprobs = logprobs;
        self
    }

    pub fn build(self) -> Choice {
        Choice {
            delta: self.delta.or(Some(Delta::Empty {})),
            finish_reason: self.finish_reason,
            index: self.index,
            logprobs: self.logprobs,
        }
    }
}

#[derive(Default)]
pub struct UsageBuilder {
    pub completion_tokens: i32,
    pub prompt_tokens: i32,
    pub total_tokens: i32,
}

impl UsageBuilder {
    pub fn completion_tokens(mut self, tokens: i32) -> Self {
        self.completion_tokens = tokens;
        self
    }

    pub fn prompt_tokens(mut self, tokens: i32) -> Self {
        self.prompt_tokens = tokens;
        self
    }

    pub fn total_tokens(mut self, tokens: i32) -> Self {
        self.total_tokens = tokens;
        self
    }

    pub fn build(self) -> Usage {
        Usage {
            prompt_tokens: self.prompt_tokens,
            completion_tokens: self.completion_tokens,
            total_tokens: self.total_tokens,
        }
    }
}

fn tool_use_block_delta_to_tool_call(
    tool_use_block_delta: &ToolUseBlockDelta,
    index: i32,
) -> ToolCall {
    ToolCall {
        id: None,
        tool_type: "function".to_string(),
        function: Some(Function {
            name: None,
            arguments: Some(tool_use_block_delta.input.clone()),
        }),
        index: Some(index),
    }
}

fn tool_use_block_start_to_tool_call(
    tool_use_block_start: &ToolUseBlockStart,
    index: i32,
) -> ToolCall {
    ToolCall {
        id: Some(tool_use_block_start.tool_use_id().to_string()),
        tool_type: "function".to_string(),
        function: Some(Function {
            name: Some(tool_use_block_start.name().to_string()),
            arguments: Some("".to_string()),
        }),
        index: Some(index),
    }
}

pub fn converse_stream_output_to_chat_completions_response_builder(
    output: &ConverseStreamOutput,
    usage_callback: Arc<dyn Fn(&aws_sdk_bedrockruntime::types::TokenUsage)>,
) -> Option<ChatCompletionsResponseBuilder> {
    let builder = ChatCompletionsResponse::builder();

    match output {
        ConverseStreamOutput::ContentBlockDelta(event) => {
            let delta = event.delta.as_ref().and_then(|d| match d {
                ContentBlockDelta::Text(text) => Some(Delta::Content {
                    content: text.clone(),
                }),
                ContentBlockDelta::ToolUse(tool_use) => {
                    let index = event.content_block_index;

                    Some(Delta::ToolCalls {
                        tool_calls: vec![tool_use_block_delta_to_tool_call(tool_use, index)],
                    })
                }
                ContentBlockDelta::ReasoningContent(ReasoningContentBlockDelta::Text(text)) => {
                    Some(Delta::Reasoning {
                        reasoning_content: text.clone(),
                    })
                }
                _ => None,
            });

            if let Some(delta) = delta {
                let choice = ChoiceBuilder::default().delta(Some(delta)).build();

                Some(builder.choice(choice))
            } else {
                None
            }
        }
        ConverseStreamOutput::ContentBlockStart(event) => {
            let delta = event.start.as_ref().and_then(|start| match start {
                ContentBlockStart::ToolUse(tool_use) => {
                    let index = event.content_block_index;
                    Some(Delta::ToolCalls {
                        tool_calls: vec![tool_use_block_start_to_tool_call(tool_use, index)],
                    })
                }
                _ => None,
            });

            if let Some(delta) = delta {
                let choice = ChoiceBuilder::default().delta(Some(delta)).build();

                Some(builder.choice(choice))
            } else {
                None
            }
        }
        ConverseStreamOutput::MessageStart(event) => {
            let delta = match event.role {
                ConversationRole::Assistant => Some(Delta::Role {
                    role: "assistant".to_string(),
                }),
                _ => None,
            };

            if let Some(delta) = delta {
                let choice = ChoiceBuilder::default().delta(Some(delta)).build();

                Some(builder.choice(choice))
            } else {
                None
            }
        }
        ConverseStreamOutput::MessageStop(event) => {
            let finish_reason = match event.stop_reason {
                StopReason::EndTurn => Some("stop".to_string()),
                StopReason::ToolUse => Some("tool_calls".to_string()),
                StopReason::MaxTokens => Some("length".to_string()),
                StopReason::StopSequence => Some("stop".to_string()),
                _ => None,
            };

            let choice = ChoiceBuilder::default()
                .finish_reason(finish_reason)
                .build();

            Some(builder.choice(choice))
        }
        ConverseStreamOutput::Metadata(event) => {
            let usage = event.usage.as_ref().map(|u| {
                usage_callback(u);

                UsageBuilder::default()
                    .completion_tokens(u.output_tokens)
                    .prompt_tokens(u.input_tokens)
                    .total_tokens(u.total_tokens)
                    .build()
            });

            let choice = ChoiceBuilder::default().build();

            Some(builder.usage(usage).choice(choice))
        }
        _ => None,
    }
}
