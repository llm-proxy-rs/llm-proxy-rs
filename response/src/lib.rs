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

#[derive(Debug, Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
