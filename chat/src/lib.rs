pub mod bedrock;
pub mod openai;
pub mod providers;

use axum::response::sse::Event;
use response::ChatCompletionsResponse;

pub const DONE_MESSAGE: &str = "[DONE]";

pub trait ProcessChatCompletionsRequest<T> {
    fn process_chat_completions_request(&self, request: &request::ChatCompletionsRequest) -> T;
}

fn create_sse_event(response: &ChatCompletionsResponse) -> anyhow::Result<Event> {
    match serde_json::to_string(response) {
        Ok(data) => Ok(Event::default().data(data)),
        Err(e) => anyhow::bail!("Failed to serialize response: {}", e),
    }
}
