use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ChatCompletionsResponse {
    pub choices: Vec<Choice>,
    pub created: Option<u64>,
    pub id: Option<String>,
    pub model: Option<String>,
    pub object: Option<String>,
    pub usage: Usage,
}

#[derive(Debug, Serialize)]
pub struct Choice {
    pub delta: Delta,
    pub finish_reason: Option<String>,
    pub index: u32,
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
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl ChatCompletionsResponse {
    pub fn builder() -> ChatCompletionsResponseBuilder {
        ChatCompletionsResponseBuilder::default()
    }
}

#[derive(Default)]
pub struct ChatCompletionsResponseBuilder {
    choices: Vec<Choice>,
    created: Option<u64>,
    id: Option<String>,
    model: Option<String>,
    object: Option<String>,
    usage: Usage,
}

impl ChatCompletionsResponseBuilder {
    pub fn choice(mut self, choice: Choice) -> Self {
        self.choices.push(choice);
        self
    }

    pub fn created(mut self, created: u64) -> Self {
        self.created = Some(created);
        self
    }

    pub fn id(mut self, id: String) -> Self {
        self.id = Some(id);
        self
    }

    pub fn model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }

    pub fn object(mut self, object: String) -> Self {
        self.object = Some(object);
        self
    }

    pub fn usage(mut self, usage: Usage) -> Self {
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
