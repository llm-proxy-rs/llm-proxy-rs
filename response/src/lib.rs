use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ChatCompletionsResponse {
    pub choices: Vec<Choice>,
    pub created: Option<u64>,
    pub id: Option<String>,
    pub model: Option<String>,
    pub object: Option<String>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Serialize)]
pub struct Choice {
    pub delta: Option<Delta>,
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
    pub completion_tokens: u32,
    pub prompt_tokens: u32,
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
    usage: Option<Usage>,
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
        self.usage = Some(usage);
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
    pub index: u32,
    pub logprobs: Option<String>,
}

impl ChoiceBuilder {
    pub fn delta(mut self, delta: Delta) -> Self {
        self.delta = Some(delta);
        self
    }

    pub fn finish_reason(mut self, reason: String) -> Self {
        self.finish_reason = Some(reason);
        self
    }

    pub fn index(mut self, index: u32) -> Self {
        self.index = index;
        self
    }

    pub fn logprobs(mut self, logprobs: String) -> Self {
        self.logprobs = Some(logprobs);
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
    pub completion_tokens: u32,
    pub prompt_tokens: u32,
    pub total_tokens: u32,
}

impl UsageBuilder {
    pub fn completion_tokens(mut self, tokens: u32) -> Self {
        self.completion_tokens = tokens;
        self
    }

    pub fn prompt_tokens(mut self, tokens: u32) -> Self {
        self.prompt_tokens = tokens;
        self
    }

    pub fn total_tokens(mut self, tokens: u32) -> Self {
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
