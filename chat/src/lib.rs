mod bedrock;
mod error;
pub mod providers;

pub trait ProcessChatCompletionsRequest<T> {
    fn process_chat_completions_request(&self) -> T;
}
