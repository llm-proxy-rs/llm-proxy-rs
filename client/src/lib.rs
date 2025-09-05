pub mod config;
pub mod event;
pub mod processor;
pub mod tool;

use anyhow::Result;
pub use event::{ChatEventHandler, DefaultChatEventHandler};
pub use processor::{
    ChatCompletionsResponseProcessor, DeltaProcessor, Processor, ResponseProcessor,
};
use request::{ChatCompletionsRequest, Message, StreamOptions, tool::ToolCall as RequestToolCall};
use std::{collections::HashMap, sync::Arc};
pub use tool::{Tool, ToolResult};

pub struct Client {
    pub config: config::ClientConfig,
    pub messages: Vec<Message>,
    pub tools: Option<HashMap<String, Box<dyn Tool>>>,
    pub chat_event_handler: Arc<dyn ChatEventHandler>,
}

impl Client {
    pub fn new(
        config: config::ClientConfig,
        chat_event_handler: Arc<dyn ChatEventHandler>,
    ) -> Self {
        Self {
            config,
            messages: Vec::new(),
            tools: None,
            chat_event_handler,
        }
    }

    pub fn config(mut self, config: config::ClientConfig) -> Self {
        self.config = config;
        self
    }

    pub fn message(mut self, message: Message) -> Self {
        self.messages.push(message);
        self
    }

    pub fn tool(mut self, tool: Box<dyn Tool>) -> Self {
        let name = tool.definition().function.name.clone();
        if let Some(ref mut tools) = self.tools {
            tools.insert(name, tool);
        } else {
            let mut tools = HashMap::new();
            tools.insert(name, tool);
            self.tools = Some(tools);
        }
        self
    }

    pub async fn send(&mut self) -> Result<()> {
        loop {
            let chat_completions_request = self.get_chat_completions_request()?;
            let url = format!("{}/chat/completions", self.config.base_url);

            let client = reqwest::Client::new();
            let response = client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&chat_completions_request)
                .send()
                .await?;

            let mut response_processor = ResponseProcessor::new(self.chat_event_handler.clone());
            response_processor.process(response).await?;

            let assistant_message_content = response_processor.get_assistant_message_content();
            let request_tool_calls = response_processor.get_request_tool_calls()?;

            let assistant_message =
                Message::assistant(&assistant_message_content, request_tool_calls.clone());

            let Some(ref tool_calls) = request_tool_calls else {
                self.messages.push(assistant_message);
                return Ok(());
            };

            self.chat_event_handler.on_tool_start(tool_calls.len())?;

            let tool_results = self.execute_tool_calls(tool_calls).await?;

            self.messages.push(assistant_message.clone());
            for result in tool_results {
                self.messages.push(result.into());
            }

            self.chat_event_handler.on_continuation()?;
        }
    }

    pub async fn execute_tool_calls(
        &self,
        tool_calls: &[RequestToolCall],
    ) -> Result<Vec<ToolResult>> {
        let mut results = Vec::new();

        for tool_call in tool_calls {
            let tool_name = &tool_call.function.name;
            let tool_args = &tool_call.function.arguments;

            self.chat_event_handler
                .on_tool_call(tool_name, Some(tool_args))?;

            if let Some(ref tools) = self.tools {
                if let Some(tool) = tools.get(tool_name) {
                    match tool.execute(Some(tool_args)) {
                        Ok(result_content) => {
                            self.chat_event_handler
                                .on_tool_result(tool_name, &result_content)?;

                            results.push(ToolResult {
                                tool_call_id: tool_call.id.clone(),
                                content: result_content,
                            });
                        }
                        Err(error) => {
                            let error_msg = error.to_string();

                            self.chat_event_handler
                                .on_tool_error(tool_name, &error_msg)?;

                            results.push(ToolResult {
                                tool_call_id: tool_call.id.clone(),
                                content: format!("Error: {}", error_msg),
                            });
                        }
                    }
                } else {
                    let error_msg = format!("Tool '{}' not found", tool_name);

                    self.chat_event_handler
                        .on_tool_error(tool_name, &error_msg)?;

                    results.push(ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        content: format!("Error: {}", error_msg),
                    });
                }
            } else {
                let error_msg = "No tools registered";

                self.chat_event_handler
                    .on_tool_error(tool_name, error_msg)?;

                results.push(ToolResult {
                    tool_call_id: tool_call.id.clone(),
                    content: format!("Error: {}", error_msg),
                });
            }
        }

        Ok(results)
    }

    pub fn get_tool_definitions(&self) -> Option<Vec<request::tool::Tool>> {
        self.tools
            .as_ref()
            .map(|tools| tools.values().map(|tool| tool.definition()).collect())
    }

    fn get_chat_completions_request(&self) -> Result<ChatCompletionsRequest> {
        Ok(ChatCompletionsRequest {
            frequency_penalty: None,
            logit_bias: None,
            max_tokens: self.config.max_tokens,
            messages: self.messages.clone(),
            model: self.config.model_id.clone(),
            n: None,
            presence_penalty: None,
            reasoning_effort: None,
            stop: None,
            stream: Some(true),
            stream_options: Some(StreamOptions {
                include_usage: true,
            }),
            temperature: self.config.temperature,
            tool_choice: None,
            tools: self.get_tool_definitions(),
            top_p: self.config.top_p,
            user: None,
        })
    }
}
