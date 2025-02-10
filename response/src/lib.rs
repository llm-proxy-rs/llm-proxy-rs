use aws_sdk_bedrockruntime::types::{
    ContentBlockDelta, ConversationRole, ConverseStreamOutput, StopReason,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ChatCompletionsResponse {
    pub choices: Vec<Choice>,
    pub created: Option<i64>,
    pub id: Option<String>,
    pub model: Option<String>,
    pub object: Option<String>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Serialize)]
pub struct Choice {
    pub delta: Option<Delta>,
    pub finish_reason: Option<String>,
    pub index: i32,
    pub logprobs: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Delta {
    Content { content: String },
    Role { role: String },
}

#[derive(Debug, Default, Serialize)]
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

impl From<ConverseStreamOutput> for ChatCompletionsResponseBuilder {
    fn from(output: ConverseStreamOutput) -> Self {
        let mut builder = ChatCompletionsResponse::builder();

        match output {
            ConverseStreamOutput::ContentBlockDelta(event) => {
                let delta = event
                    .delta
                    .and_then(|d| match d {
                        ContentBlockDelta::Text(text) => Some(text),
                        _ => None,
                    })
                    .map(|content| Delta::Content { content });

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
                        _ => None,
                    })
                    .build();

                builder = builder.choice(choice);
            }
            ConverseStreamOutput::Metadata(event) => {
                let usage = event.usage.map(|u| {
                    UsageBuilder::default()
                        .completion_tokens(u.output_tokens)
                        .prompt_tokens(u.input_tokens)
                        .total_tokens(u.total_tokens)
                        .build()
                });

                builder = builder.usage(usage);
            }
            _ => {}
        }

        builder
    }
}
