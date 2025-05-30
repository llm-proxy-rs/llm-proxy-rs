use aws_sdk_bedrockruntime::types::{
    ContentBlockDelta, ContentBlockStart, ConversationRole, ConverseStreamOutput, StopReason,
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
    FunctionCall { function_call: FunctionCall },
    Empty {},
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
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
            delta: self.delta,
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

pub fn converse_stream_output_to_chat_completions_response_builder(
    output: &ConverseStreamOutput,
    usage_callback: Arc<dyn Fn(&Usage)>,
) -> ChatCompletionsResponseBuilder {
    let mut builder = ChatCompletionsResponse::builder();

    match output {
        ConverseStreamOutput::ContentBlockDelta(event) => {
            let delta = event.delta.as_ref().and_then(|d| match d {
                ContentBlockDelta::Text(text) => Some(Delta::Content {
                    content: text.clone(),
                }),
                ContentBlockDelta::ToolUse(tool_use_delta) => {
                    // Handle tool use delta - typically this is for streaming tool arguments
                    let args = serde_json::to_string(&tool_use_delta.input).unwrap_or_default();
                    Some(Delta::ToolCalls {
                        tool_calls: vec![ToolCall {
                            id: "".to_string(), // Will be set in ContentBlockStart
                            r#type: "function".to_string(),
                            function: FunctionCall {
                                name: "".to_string(), // Will be set in ContentBlockStart
                                arguments: args,
                            },
                        }],
                    })
                }
                _ => None,
            });

            let choice = ChoiceBuilder::default()
                .delta(delta)
                .index(event.content_block_index)
                .build();

            builder = builder.choice(choice);
        }
        ConverseStreamOutput::ContentBlockStart(event) => {
            let delta = event.start.as_ref().and_then(|start| match start {
                ContentBlockStart::ToolUse(tool_start) => {
                    Some(Delta::ToolCalls {
                        tool_calls: vec![ToolCall {
                            id: tool_start.tool_use_id.clone(),
                            r#type: "function".to_string(),
                            function: FunctionCall {
                                name: tool_start.name.clone(),
                                arguments: "".to_string(), // Arguments will come in delta events
                            },
                        }],
                    })
                }
                _ => None,
            });

            let choice = ChoiceBuilder::default()
                .delta(delta)
                .index(event.content_block_index)
                .build();

            builder = builder.choice(choice);
        }
        ConverseStreamOutput::MessageStart(event) => {
            let choice = ChoiceBuilder::default()
                .delta(match event.role {
                    ConversationRole::Assistant => Some(Delta::Role {
                        role: "assistant".to_string(),
                    }),
                    _ => None,
                })
                .build();

            builder = builder.choice(choice);
        }
        ConverseStreamOutput::MessageStop(event) => {
            let choice = ChoiceBuilder::default()
                .finish_reason(match event.stop_reason {
                    StopReason::EndTurn => Some("stop".to_string()),
                    StopReason::ToolUse => Some("tool_calls".to_string()),
                    _ => None,
                })
                .build();

            builder = builder.choice(choice);
        }
        ConverseStreamOutput::Metadata(event) => {
            let usage = event.usage.as_ref().map(|u| {
                let usage = UsageBuilder::default()
                    .completion_tokens(u.output_tokens)
                    .prompt_tokens(u.input_tokens)
                    .total_tokens(u.total_tokens)
                    .build();

                usage_callback(&usage);

                usage
            });

            builder = builder.usage(usage);
        }
        _ => {}
    }

    builder
}
