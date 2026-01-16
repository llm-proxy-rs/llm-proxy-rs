pub mod bedrock;
pub mod converters;
pub mod providers;

use axum::response::sse::Event;
use response::ChatCompletionsResponse;

pub const DONE_MESSAGE: &str = "[DONE]";

fn create_sse_event(response: &ChatCompletionsResponse) -> anyhow::Result<Event> {
    match serde_json::to_string(response) {
        Ok(data) => Ok(Event::default().data(data)),
        Err(e) => anyhow::bail!("Failed to serialize response: {}", e),
    }
}
