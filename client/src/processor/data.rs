use anyhow::Result;
use response::ChatCompletionsResponse;
use serde_json;
use std::sync::Arc;

use super::{ChatCompletionsResponseProcessor, Processor};
use crate::event::ChatEventHandler;

/// A processor for handling Server-Sent Events (SSE) data chunks
/// that contain JSON responses in the format "data: <json>"
pub struct DataProcessor {
    chat_completions_response_processor: ChatCompletionsResponseProcessor,
}

impl Processor<Arc<dyn ChatEventHandler>, String, bool> for DataProcessor {
    fn new(chat_event_handler: Arc<dyn ChatEventHandler>) -> Self {
        let chat_completions_response_processor =
            ChatCompletionsResponseProcessor::new(chat_event_handler);
        Self {
            chat_completions_response_processor,
        }
    }

    fn process(&mut self, data_chunk: &String) -> Result<bool> {
        for line in data_chunk.lines() {
            if let Some(json_payload) = line.strip_prefix("data: ") {
                // Check for stream completion marker
                if json_payload == "[DONE]" {
                    return Ok(true);
                }

                // Parse and handle valid JSON responses
                if let Ok(chat_completions_response) =
                    serde_json::from_str::<ChatCompletionsResponse>(json_payload)
                {
                    self.chat_completions_response_processor
                        .process(&chat_completions_response)?;
                }
            }
        }
        Ok(false)
    }
}
