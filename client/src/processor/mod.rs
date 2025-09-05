use anyhow::Result;

pub mod chat_completions_response;
pub mod data;
pub mod delta;
pub mod response;

pub use chat_completions_response::ChatCompletionsResponseProcessor;
pub use data::DataProcessor;
pub use delta::DeltaProcessor;
pub use response::ResponseProcessor;

#[async_trait::async_trait]
pub trait Processor<T, U, R = ()> {
    fn new(config: T) -> Self;
    async fn process(&mut self, input: U) -> Result<R>;
}
