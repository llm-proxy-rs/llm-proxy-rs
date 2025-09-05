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
        while let Some(chunk_result) = stream.next().await {
            let chunk_bytes = chunk_result?;
            let chunk_string = String::from_utf8_lossy(&chunk_bytes).to_string();
            if self.data_processor.process(chunk_string).await? {
                break;
            }
        }
        Ok(())
    }
}
