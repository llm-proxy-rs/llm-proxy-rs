use anyhow::Result;
use response::ChatCompletionsResponse;
use serde_json;
use std::sync::Arc;

use super::{ChatCompletionsResponseProcessor, Processor};
use crate::event::ChatEventHandler;

pub struct DataProcessor {
    chat_completions_response_processor: ChatCompletionsResponseProcessor,
}

#[async_trait::async_trait]
impl Processor<Arc<dyn ChatEventHandler>, String, bool> for DataProcessor {
    fn new(chat_event_handler: Arc<dyn ChatEventHandler>) -> Self {
        let chat_completions_response_processor =
            ChatCompletionsResponseProcessor::new(chat_event_handler);
        Self {
            chat_completions_response_processor,
        }
    }

    async fn process(&mut self, data: String) -> Result<bool> {
        for line in data.lines() {
            if let Some(json_payload) = line.strip_prefix("data: ") {
                if json_payload == "[DONE]" {
                    return Ok(true);
                }

                if let Ok(chat_completions_response) =
                    serde_json::from_str::<ChatCompletionsResponse>(json_payload)
                {
                    self.chat_completions_response_processor
                        .process(chat_completions_response)
                        .await?;
                }
            }
        }
        Ok(false)
    }
}

impl DataProcessor {
    pub fn get_assistant_message_content(&self) -> String {
        self.chat_completions_response_processor
            .get_assistant_message_content()
    }

    pub fn get_request_tool_calls(&self) -> Result<Option<Vec<request::tool::ToolCall>>> {
        self.chat_completions_response_processor
            .get_request_tool_calls()
    }
}
