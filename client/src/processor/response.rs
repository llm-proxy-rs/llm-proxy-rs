use anyhow::Result;
use futures_util::StreamExt;
use std::sync::Arc;

use super::{DataProcessor, Processor};
use crate::event::ChatEventHandler;

pub struct ResponseProcessor {
    data_processor: DataProcessor,
}

#[async_trait::async_trait]
impl Processor<Arc<dyn ChatEventHandler>, reqwest::Response> for ResponseProcessor {
    fn new(chat_event_handler: Arc<dyn ChatEventHandler>) -> Self {
        let data_processor = DataProcessor::new(chat_event_handler);
        Self { data_processor }
    }

    async fn process(&mut self, response: reqwest::Response) -> Result<()> {
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = String::from_utf8_lossy(&chunk?).to_string();
            if self.data_processor.process(chunk).await? {
                break;
            }
        }
        Ok(())
    }
}

impl ResponseProcessor {
    pub fn get_assistant_message_content(&self) -> String {
        self.data_processor.get_assistant_message_content()
    }

    pub fn get_request_tool_calls(&self) -> anyhow::Result<Option<Vec<request::tool::ToolCall>>> {
        self.data_processor.get_request_tool_calls()
    }
}
