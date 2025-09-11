use anyhow::Result;
use response::ChatCompletionsResponse;
use std::sync::Arc;
use tracing::info;

use super::{ChatCompletionsResponseProcessor, Processor};
use crate::event::ChatEventHandler;

pub struct DataProcessor {
    chat_completions_response_processor: ChatCompletionsResponseProcessor,
    data_buffer: String,
}

#[async_trait::async_trait]
impl Processor<Arc<dyn ChatEventHandler>, String, bool> for DataProcessor {
    fn new(chat_event_handler: Arc<dyn ChatEventHandler>) -> Self {
        let chat_completions_response_processor =
            ChatCompletionsResponseProcessor::new(chat_event_handler);
        Self {
            chat_completions_response_processor,
            data_buffer: String::new(),
        }
    }

    async fn process(&mut self, data: String) -> Result<bool> {
        self.data_buffer.push_str(&data);

        while let Some(newline_pos) = self.data_buffer.find('\n') {
            let line = self.data_buffer.drain(..=newline_pos).collect::<String>();
            let line = line.trim_end();

            if let Some(json_payload) = line.strip_prefix("data: ") {
                if json_payload == "[DONE]" {
                    return Ok(true);
                }

                if json_payload.contains("tool_calls") {
                    info!("SSE event: {}", json_payload);
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
    pub fn get_assistant_message_content(&self) -> Option<String> {
        self.chat_completions_response_processor
            .get_assistant_message_content()
    }

    pub fn get_request_tool_calls(&self) -> Result<Option<Vec<request::tool::ToolCall>>> {
        self.chat_completions_response_processor
            .get_request_tool_calls()
    }
}
