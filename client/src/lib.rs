//! # LLM Proxy Client
//!
//! A comprehensive streaming chat client for LLM proxy services with built-in tool support.
//!
//! ## Features
//!
//! - **Streaming Support**: Real-time streaming of chat completions
//! - **Tool Integration**: Built-in support for function calling with tools
//! - **Error Handling**: Comprehensive error handling with `anyhow`
//! - **Flexible Architecture**: Builder pattern for easy configuration
//! - **Tool Support**: Easy integration of custom tools via builder pattern
//!
//! ## Quick Start
//!
//! ```rust
//! use client::{Chat, config::ClientConfig};
//! use request::Message;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create a new chat client
//!     let mut client = Chat::new("http://localhost:8080", "your-model-id");
//!
//!     // Add messages using convenience methods and send
//!     client
//!         .message(Message::system("You are a helpful assistant."))
//!         .message(Message::user("Hello, world!"))
//!         .send()
//!         .await?;
//!
//!     // You can chain multiple messages
//!     client
//!         .message(Message::user("How are you?"))
//!         .message(Message::user("What's the weather like?"))
//!         .send()
//!         .await?;
//!
//!     Ok(())
//! }
//! ```

use anyhow::{Result, bail};
use reqwest::Client;
use std::collections::HashMap;

use request::tool::Tool as RequestTool;
use request::{ChatCompletionsRequest, Message, ToolCall};
use response::ToolCall as ResponseToolCall;
use tool::{Tool, ToolResult};

pub mod config;
pub mod delta;
pub mod handler;
pub mod tool;

use delta::DeltaProcessor;
pub use handler::ChatEventHandler;

pub struct Chat {
    pub config: config::ClientConfig,
    pub messages: Vec<Message>,
    pub tools: Option<HashMap<String, Box<dyn Tool>>>,
}

pub struct ChatBuilder {
    pub config: config::ClientConfig,
    pub messages: Vec<Message>,
    pub tools: Option<HashMap<String, Box<dyn Tool>>>,
}

impl ChatBuilder {
    pub fn new(config: config::ClientConfig) -> Self {
        Self {
            config,
            messages: Vec::new(),
            tools: None,
        }
    }

    pub fn message(mut self, message: Message) -> Self {
        self.messages.push(message);
        self
    }

    pub fn tool(mut self, tool: Box<dyn Tool>) -> Self {
        let name = tool.definition().function.name.clone();

        // Initialize tools HashMap if it doesn't exist
        if self.tools.is_none() {
            self.tools = Some(HashMap::new());
        }

        self.tools.as_mut().unwrap().insert(name, tool);
        self
    }

    pub fn build(self) -> Chat {
        Chat {
            config: self.config,
            messages: self.messages,
            tools: self.tools,
        }
    }
}

impl Chat {
    pub fn builder(config: config::ClientConfig) -> ChatBuilder {
        ChatBuilder::new(config)
    }

    /// Create a new Chat instance without tools
    pub fn new(base_url: &str, model_id: &str) -> Self {
        let config = config::ClientConfig {
            base_url: base_url.to_string(),
            model_id: model_id.to_string(),
        };

        Self {
            config,
            messages: Vec::new(),
            tools: None,
        }
    }

    /// Get tool definitions for API requests
    pub fn get_tool_definitions(&self) -> Vec<RequestTool> {
        match &self.tools {
            Some(tools) => tools.values().map(|tool| tool.definition()).collect(),
            None => Vec::new(),
        }
    }

    /// Execute a tool by name with optional arguments
    pub fn execute_tool(&self, name: &str, args: Option<&str>) -> Result<String> {
        match &self.tools {
            Some(tools) => match tools.get(name) {
                Some(tool) => tool.execute(args),
                None => bail!("Unknown tool: {}", name),
            },
            None => bail!("No tools available"),
        }
    }

    /// Check if tools are available
    pub fn has_tools(&self) -> bool {
        self.tools.is_some()
    }

    /// Get the number of available tools
    pub fn tool_count(&self) -> usize {
        match &self.tools {
            Some(tools) => tools.len(),
            None => 0,
        }
    }

    /// Send a simple chat message and handle the streaming response
    pub async fn chat(&self, message: &str, handler: &mut dyn ChatEventHandler) -> Result<()> {
        let user_message = Message::User {
            contents: Some(request::Contents::String(message.to_string())),
        };

        let mut messages = self.messages.clone();
        messages.push(user_message);

        let request = ChatCompletionsRequest {
            model: self.config.model_id.clone(),
            messages,
            tools: if self.has_tools() {
                Some(self.get_tool_definitions())
            } else {
                None
            },
            stream: Some(true),
            max_tokens: Some(1000),
            temperature: Some(0.1),
            top_p: Some(0.1),
            stop: None,
            frequency_penalty: None,
            presence_penalty: None,
            logit_bias: None,
            user: None,
            n: None,
            stream_options: None,
            tool_choice: None,
            reasoning_effort: None,
        };

        self.chat_completions_stream(request, handler).await
    }

    /// Send a chat completion request and handle the streaming response
    pub async fn chat_completions_stream(
        &self,
        mut request: ChatCompletionsRequest,
        handler: &mut dyn ChatEventHandler,
    ) -> Result<()> {
        loop {
            let assistant_message = self
                .send_request_and_get_assistant_message(&request, handler)
                .await?;

            // If no message or no tool calls, we're done
            let assistant_msg = match assistant_message {
                Some(msg) => msg,
                None => break,
            };

            let tool_calls = match &assistant_msg {
                Message::Assistant {
                    tool_calls: Some(tool_calls),
                    ..
                } => {
                    // Extract ResponseToolCall from request ToolCall for execution
                    self.extract_response_tool_calls(tool_calls)
                }
                _ => break, // No tool calls, conversation is complete
            };

            if tool_calls.is_empty() {
                break;
            }

            let tool_results = self.execute_tool_calls(&tool_calls, handler).await?;

            // Add the assistant message and tool results to conversation
            request.messages.push(assistant_msg);
            for result in tool_results {
                request.messages.push(result.into());
            }

            handler.on_continuation()?;
        }

        Ok(())
    }

    /// Execute a tool call and return the result
    fn execute_tool_call(&self, tool_call: &ResponseToolCall) -> Result<ToolResult> {
        let function = tool_call
            .function
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tool call missing function"))?;

        let name = function
            .name
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tool call missing function name"))?;

        let id = tool_call
            .id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tool call missing id"))?
            .clone();

        let args_opt = function.arguments.clone();
        let result = self.execute_tool(name, args_opt.as_deref())?;

        Ok(ToolResult {
            id,
            name: name.clone(),
            result,
        })
    }

    /// Execute all tool calls and return results
    async fn execute_tool_calls(
        &self,
        tool_calls: &[ResponseToolCall],
        handler: &mut dyn ChatEventHandler,
    ) -> Result<Vec<ToolResult>> {
        handler.on_tool_start(tool_calls.len())?;
        let mut tool_results = Vec::new();

        for tool_call in tool_calls.iter() {
            // Get tool call information
            if let Some(function) = &tool_call.function
                && let Some(name) = &function.name
            {
                let args = function.arguments.as_deref();
                handler.on_tool_call(name, args)?;
            }

            // Execute and handle result
            match self.execute_tool_call(tool_call) {
                Ok(result) => {
                    handler.on_tool_result(&result.name, &result.result)?;
                    tool_results.push(result);
                }
                Err(e) => {
                    let tool_name = tool_call
                        .function
                        .as_ref()
                        .and_then(|f| f.name.as_ref())
                        .map_or("unknown", |name| name.as_str());
                    handler.on_tool_error(tool_name, &e.to_string())?;
                }
            }
        }

        Ok(tool_results)
    }

    /// Extract ResponseToolCall from request ToolCall for execution
    fn extract_response_tool_calls(&self, tool_calls: &[ToolCall]) -> Vec<ResponseToolCall> {
        tool_calls
            .iter()
            .map(|tc| ResponseToolCall {
                id: Some(tc.id.clone()),
                tool_type: tc.tool_type.clone(),
                function: Some(response::Function {
                    name: Some(tc.function.name.clone()),
                    arguments: Some(tc.function.arguments.clone()),
                }),
                index: None, // Not needed for execution
            })
            .collect()
    }

    /// Send request and get assistant message from response
    async fn send_request_and_get_assistant_message(
        &self,
        request: &ChatCompletionsRequest,
        handler: &mut dyn ChatEventHandler,
    ) -> Result<Option<Message>> {
        let url = format!("{}/chat/completions", self.config.base_url);

        let response = self.send_http_request(&url, request).await?;
        let assistant_message = self.process_streaming_response(response, handler).await?;

        Ok(assistant_message)
    }

    /// Send HTTP request and return response
    async fn send_http_request(
        &self,
        url: &str,
        request: &ChatCompletionsRequest,
    ) -> Result<reqwest::Response> {
        let client = Client::new();
        let response = client.post(url).json(request).send().await?;

        if !response.status().is_success() {
            bail!("Request failed with status: {}", response.status());
        }

        Ok(response)
    }

    /// Process streaming response and build assistant message using DeltaProcessor
    async fn process_streaming_response(
        &self,
        response: reqwest::Response,
        handler: &mut dyn ChatEventHandler,
    ) -> Result<Option<Message>> {
        let mut processor = DeltaProcessor::new(handler);
        processor.process_streaming_response(response).await?;
        processor.build_assistant_message()
    }

    /// Add a message to the conversation
    pub fn message(&mut self, message: Message) -> &mut Self {
        self.messages.push(message);
        self
    }

    /// Send all messages and handle the streaming response
    pub async fn send(&self, handler: &mut dyn ChatEventHandler) -> Result<()> {
        if self.messages.is_empty() {
            bail!("No messages to send");
        }

        let request = ChatCompletionsRequest {
            model: self.config.model_id.clone(),
            messages: self.messages.clone(),
            tools: if self.has_tools() {
                Some(self.get_tool_definitions())
            } else {
                None
            },
            stream: Some(true),
            max_tokens: Some(1000),
            temperature: Some(0.1),
            top_p: Some(0.1),
            stop: None,
            frequency_penalty: None,
            presence_penalty: None,
            logit_bias: None,
            user: None,
            n: None,
            stream_options: None,
            tool_choice: None,
            reasoning_effort: None,
        };

        self.chat_completions_stream(request, handler).await
    }
}
