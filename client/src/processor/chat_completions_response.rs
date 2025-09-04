use anyhow::Result;
use response::ChatCompletionsResponse;
use std::sync::Arc;

use super::{DeltaProcessor, Processor};
use crate::event::ChatEventHandler;

pub struct ChatCompletionsResponseProcessor {
    chat_event_handler: Arc<dyn ChatEventHandler>,
    delta_processor: DeltaProcessor,
}

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

    fn process(&mut self, chat_completions_response: &ChatCompletionsResponse) -> Result<()> {
        if let Some(usage) = &chat_completions_response.usage {
            self.chat_event_handler.on_usage(
                usage.prompt_tokens,
                usage.completion_tokens,
                usage.total_tokens,
            )?;
        }

        for choice in &chat_completions_response.choices {
            if let Some(delta) = &choice.delta {
                self.delta_processor.process(delta)?;
            }
            if let Some(finish_reason) = &choice.finish_reason {
                self.chat_event_handler.on_finish(finish_reason)?;
            }
        }
        Ok(())
    }
}
