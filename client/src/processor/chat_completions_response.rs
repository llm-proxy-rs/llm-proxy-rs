use anyhow::Result;
use response::ChatCompletionsResponse;
use std::sync::Arc;

use super::{DeltaProcessor, Processor};
use crate::event::ChatEventHandler;

pub struct ChatCompletionsResponseProcessor {
    chat_event_handler: Arc<dyn ChatEventHandler>,
    delta_processor: DeltaProcessor,
}

#[async_trait::async_trait]
impl Processor<Arc<dyn ChatEventHandler>, ChatCompletionsResponse>
    for ChatCompletionsResponseProcessor
{
    fn new(chat_event_handler: Arc<dyn ChatEventHandler>) -> Self {
        let delta_processor = DeltaProcessor::new(chat_event_handler.clone());
        Self {
            chat_event_handler,
            delta_processor,
        }
    }

    async fn process(&mut self, chat_completions_response: ChatCompletionsResponse) -> Result<()> {
        if let Some(usage) = &chat_completions_response.usage {
            self.chat_event_handler
                .on_usage(
                    usage.prompt_tokens,
                    usage.completion_tokens,
                    usage.total_tokens,
                )
                .await?;
        }

        for choice in &chat_completions_response.choices {
            if let Some(delta) = choice.delta.clone() {
                self.delta_processor.process(delta).await?;
            }
            if let Some(finish_reason) = &choice.finish_reason {
                self.chat_event_handler.on_finish(finish_reason).await?;
            }
        }
        Ok(())
    }
}

impl ChatCompletionsResponseProcessor {
    pub fn get_assistant_message_content(&self) -> Option<String> {
        self.delta_processor.get_assistant_message_content()
    }

    pub fn get_request_tool_calls(&self) -> Result<Option<Vec<request::tool::ToolCall>>> {
        self.delta_processor.get_request_tool_calls()
    }
}
