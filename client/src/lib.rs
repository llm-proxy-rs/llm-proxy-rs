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
use std::{
    collections::HashMap,
    io::{self, Write},
};
use tokio_stream::StreamExt;

use request::tool::Tool as RequestTool;
use request::{ChatCompletionsRequest, Message, ToolCall, Contents};
use response::{ChatCompletionsResponse, Delta, ToolCall as ResponseToolCall};
use tool::{Tool, ToolResult};

pub mod config;
pub mod tool;

/// Trait for handling chat responses
pub trait ResponseHandler {
    /// Handle a role announcement (e.g., "Assistant:", "User:")
    fn on_role(&mut self, role: &str) -> Result<()>;

    /// Handle streaming content
    fn on_content(&mut self, content: &str) -> Result<()>;

    /// Handle reasoning content
    fn on_reasoning(&mut self, reasoning: &str) -> Result<()>;

    /// Handle finish reason
    fn on_finish(&mut self, reason: &str) -> Result<()>;

    /// Handle usage information
    fn on_usage(
        &mut self,
        prompt_tokens: u32,
        completion_tokens: u32,
        total_tokens: u32,
    ) -> Result<()>;

    /// Handle tool execution start
    fn on_tool_start(&mut self, tool_count: usize) -> Result<()>;

    /// Handle individual tool call
    fn on_tool_call(&mut self, name: &str, args: Option<&str>) -> Result<()>;

    /// Handle tool execution result
    fn on_tool_result(&mut self, name: &str, result: &str) -> Result<()>;

    /// Handle tool execution error
    fn on_tool_error(&mut self, name: &str, error: &str) -> Result<()>;

    /// Handle continuation message
    fn on_continuation(&mut self) -> Result<()>;
}

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
    pub async fn chat(&self, message: &str, handler: &mut dyn ResponseHandler) -> Result<()> {
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
        handler: &mut dyn ResponseHandler,
    ) -> Result<()> {
        loop {
            let assistant_message = self.send_request_and_get_assistant_message(&request).await?;

            // If no message or no tool calls, we're done
            let assistant_msg = match assistant_message {
                Some(msg) => msg,
                None => break,
            };

            let tool_calls = match &assistant_msg {
                Message::Assistant { tool_calls: Some(tool_calls), .. } => {
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
        handler: &mut dyn ResponseHandler,
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

    /// Convert response tool call to request tool call
    fn convert_to_request_tool_call(&self, tc: ResponseToolCall) -> ToolCall {
        let function = tc.function.as_ref().unwrap();
        let default_args = "{}".to_string();
        let args = function.arguments.as_ref().unwrap_or(&default_args);
        let arguments = if args.trim().is_empty() { "{}" } else { args }.to_string();

        ToolCall {
            id: tc.id.unwrap_or_default(),
            tool_type: tc.tool_type,
            function: request::FunctionCall {
                name: function.name.as_ref().unwrap().clone(),
                arguments,
            },
        }
    }

    /// Print debug information about the request
    fn print_debug_info(&self, request: &ChatCompletionsRequest) {
        println!("\n[Continuing conversation with tool results...]");
        println!(
            "[DEBUG] Total messages in request: {}",
            request.messages.len()
        );

        for (i, msg) in request.messages.iter().enumerate() {
            match msg {
                Message::User { .. } => println!("  Message {}: User", i),
                Message::Assistant { tool_calls, .. } => {
                    if let Some(tc) = tool_calls {
                        println!("  Message {}: Assistant with {} tool calls", i, tc.len());
                    } else {
                        println!("  Message {}: Assistant (no tool calls)", i);
                    }
                }
                Message::Tool { tool_call_id, .. } => {
                    println!("  Message {}: Tool result for {:?}", i, tool_call_id);
                }
                Message::System { .. } => println!("  Message {}: System", i),
            }
        }
    }

    /// Send request and get assistant message from response
    async fn send_request_and_get_assistant_message(
        &self,
        request: &ChatCompletionsRequest,
    ) -> Result<Option<Message>> {
        let url = format!("{}/chat/completions", self.config.base_url);

        let response = self.send_http_request(&url, request).await?;
        let assistant_message = self.process_streaming_response(response).await?;

        println!(); // Final newline
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

    /// Process streaming response and build assistant message directly
    async fn process_streaming_response(
        &self,
        response: reqwest::Response,
    ) -> Result<Option<Message>> {
        let mut stream = response.bytes_stream();
        let mut content_buffer = String::new();
        let mut tool_calls_map: HashMap<String, ResponseToolCall> = HashMap::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let chunk_str = String::from_utf8_lossy(&chunk);

            let (should_break, updated_tool_calls_map) =
                self.process_chunk_lines(&chunk_str, &mut content_buffer, &tool_calls_map)?;
            tool_calls_map = updated_tool_calls_map;

            if should_break {
                break;
            }
        }

        let tool_calls: Vec<ResponseToolCall> = tool_calls_map.into_values().collect();
        
        // If no content and no tool calls, return None
        if content_buffer.trim().is_empty() && tool_calls.is_empty() {
            return Ok(None);
        }

        // Build the assistant message directly
        let contents = if content_buffer.trim().is_empty() {
            None
        } else {
            Some(Contents::String(content_buffer))
        };

        let request_tool_calls = if tool_calls.is_empty() {
            None
        } else {
            Some(
                tool_calls
                    .into_iter()
                    .map(|tc| self.convert_to_request_tool_call(tc))
                    .collect(),
            )
        };

        Ok(Some(Message::Assistant {
            contents,
            tool_calls: request_tool_calls,
        }))
    }

    /// Process individual chunk lines
    fn process_chunk_lines(
        &self,
        chunk_str: &str,
        content_buffer: &mut String,
        tool_calls_map: &HashMap<String, ResponseToolCall>,
    ) -> Result<(bool, HashMap<String, ResponseToolCall>)> {
        let mut updated_tool_calls_map = tool_calls_map.clone();

        for line in chunk_str.lines() {
            if let Some(json_str) = line.strip_prefix("data: ") {
                if json_str == "[DONE]" {
                    return Ok((true, updated_tool_calls_map));
                }

                if let Ok(response) = serde_json::from_str::<ChatCompletionsResponse>(json_str) {
                    updated_tool_calls_map = self.handle_response_chunk(
                        &response,
                        content_buffer,
                        &updated_tool_calls_map,
                    )?;
                }
            }
        }
        Ok((false, updated_tool_calls_map))
    }

    /// Handle individual response chunks
    fn handle_response_chunk(
        &self,
        response: &ChatCompletionsResponse,
        content_buffer: &mut String,
        tool_calls_map: &HashMap<String, ResponseToolCall>,
    ) -> Result<HashMap<String, ResponseToolCall>> {
        let mut updated_tool_calls_map = tool_calls_map.clone();

        for choice in &response.choices {
            if let Some(delta) = &choice.delta {
                updated_tool_calls_map =
                    self.process_delta(delta, content_buffer, &updated_tool_calls_map)?;
            }

            if let Some(finish_reason) = &choice.finish_reason {
                println!("\n[Finished]: {}", finish_reason);
            }
        }

        Ok(updated_tool_calls_map)
    }

    /// Process delta content from response
    fn process_delta(
        &self,
        delta: &Delta,
        content_buffer: &mut String,
        tool_calls_map: &HashMap<String, ResponseToolCall>,
    ) -> Result<HashMap<String, ResponseToolCall>> {
        match delta {
            Delta::Role { role } => {
                println!("\n[{}]: ", role);
                io::stdout().flush()?;
                Ok(tool_calls_map.clone())
            }
            Delta::Content { content } => {
                print!("{}", content);
                content_buffer.push_str(content);
                io::stdout().flush()?;
                Ok(tool_calls_map.clone())
            }
            Delta::ToolCalls { tool_calls } => {
                self.process_tool_calls_delta(tool_calls, tool_calls_map)
            }
            Delta::Reasoning { reasoning_content } => {
                println!("\n[Reasoning]: {}", reasoning_content);
                Ok(tool_calls_map.clone())
            }
            Delta::Empty {} => {
                // No-op for empty deltas
                Ok(tool_calls_map.clone())
            }
        }
    }

    /// Process tool calls from delta
    fn process_tool_calls_delta(
        &self,
        tool_calls: &[ResponseToolCall],
        tool_calls_map: &HashMap<String, ResponseToolCall>,
    ) -> Result<HashMap<String, ResponseToolCall>> {
        let mut updated_map = tool_calls_map.clone();
        for tool_call in tool_calls {
            updated_map = self.merge_or_insert_tool_call(tool_call, &updated_map);
        }
        Ok(updated_map)
    }

    /// Merge or insert tool call into the map
    fn merge_or_insert_tool_call(
        &self,
        tool_call: &ResponseToolCall,
        tool_calls_map: &HashMap<String, ResponseToolCall>,
    ) -> HashMap<String, ResponseToolCall> {
        let key = tool_call
            .index
            .map(|i| i.to_string())
            .unwrap_or_else(|| "default".to_string());

        let mut updated_map = tool_calls_map.clone();

        match tool_calls_map.get(&key) {
            Some(existing) => {
                let merged_tool_call = self.merge_tool_call(existing, tool_call);
                updated_map.insert(key, merged_tool_call);
            }
            None => {
                updated_map.insert(key, tool_call.clone());
            }
        }

        updated_map
    }

    /// Merge new tool call data into existing tool call
    fn merge_tool_call(
        &self,
        existing: &ResponseToolCall,
        new_call: &ResponseToolCall,
    ) -> ResponseToolCall {
        println!(
            "[DEBUG] Merging tool call - Existing: {:?}, New: {:?}",
            existing, new_call
        );

        let mut merged = existing.clone();

        if new_call.id.is_some() {
            merged.id = new_call.id.clone();
        }

        if let Some(new_func) = &new_call.function {
            if let Some(existing_func) = &existing.function {
                let mut merged_func = existing_func.clone();

                if new_func.name.is_some() {
                    merged_func.name = new_func.name.clone();
                }

                if let Some(new_args) = &new_func.arguments {
                    println!(
                        "[DEBUG] New arguments: '{}', is_empty: {}",
                        new_args,
                        new_args.trim().is_empty()
                    );
                    if !new_args.trim().is_empty() {
                        if let Some(existing_args) = &existing_func.arguments {
                            println!(
                                "[DEBUG] Appending to existing args: '{}' + '{}'",
                                existing_args, new_args
                            );
                            merged_func.arguments = Some(format!("{}{}", existing_args, new_args));
                        } else {
                            println!("[DEBUG] Setting new args: '{}'", new_args);
                            merged_func.arguments = Some(new_args.clone());
                        }
                    } else {
                        println!("[DEBUG] Skipping empty arguments");
                    }
                }

                merged.function = Some(merged_func);
            } else {
                merged.function = Some(new_func.clone());
            }
        }

        println!("[DEBUG] After merge: {:?}", merged);
        merged
    }

    /// Add a message to the conversation
    pub fn message(&mut self, message: Message) -> &mut Self {
        self.messages.push(message);
        self
    }

    /// Send all messages and handle the streaming response
    pub async fn send(&self, handler: &mut dyn ResponseHandler) -> Result<()> {
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
