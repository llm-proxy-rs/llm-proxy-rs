mod bedrock;
pub mod providers;

pub trait ProcessChatCompletionsRequest<T> {
    fn process_chat_completions_request(&self, request: &request::ChatCompletionsRequest) -> T;
}
