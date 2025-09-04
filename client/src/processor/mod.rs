use anyhow::Result;

pub mod chat_completions_response;
pub mod data;
pub mod delta;

pub use chat_completions_response::ChatCompletionsResponseProcessor;
pub use data::DataProcessor;
pub use delta::DeltaProcessor;

pub trait Processor<T, U, R = ()> {
    fn new(config: T) -> Self;
    fn process(&mut self, input: &U) -> Result<R>;
}
