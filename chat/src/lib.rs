pub mod bedrock;
pub mod openai;
pub mod providers;

pub const DONE_MESSAGE: &str = "[DONE]";

pub trait ProcessChatCompletionsRequest<T> {
    fn process_chat_completions_request(&self, request: &request::ChatCompletionsRequest) -> T;
}
