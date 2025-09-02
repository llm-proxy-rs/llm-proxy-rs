//! Delta processing module for streaming chat responses

use anyhow::Result;
use tokio_stream::StreamExt;

use crate::ChatEventHandler;
use request::{Contents, Message, ToolCall, FunctionCall};
use response::{ChatCompletionsResponse, Delta, ToolCall as ResponseToolCall};

pub struct DeltaProcessor<'a> {
    handler: &'a mut dyn ChatEventHandler,
    content_buffer: String,
    tool_call_chunks: Vec<ResponseToolCall>,
}

impl<'a> DeltaProcessor<'a> {
    pub fn new(handler: &'a mut dyn ChatEventHandler) -> Self {
        Self {
            handler,
            content_buffer: String::new(),
            tool_call_chunks: Vec::new(),
        }
    }

    pub async fn process_streaming_response(&mut self, response: reqwest::Response) -> Result<()> {
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk_bytes = chunk?;
            let chunk = String::from_utf8_lossy(&chunk_bytes);
            if self.process_chunk(&chunk)? {
                break;
            }
        }
        Ok(())
    }

    pub fn build_assistant_message(&self) -> Result<Option<Message>> {
        let tool_calls = self.build_tool_calls_from_chunks()?;

        // If no content and no tool calls, return None
        if self.content_buffer.trim().is_empty() && tool_calls.is_empty() {
            return Ok(None);
        }

        let contents = if self.content_buffer.trim().is_empty() {
            None
        } else {
            Some(Contents::String(self.content_buffer.clone()))
        };

        let tool_calls_option = if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        };

        Ok(Some(Message::Assistant {
            contents,
            tool_calls: tool_calls_option,
        }))
    }

    fn process_chunk(&mut self, chunk: &str) -> Result<bool> {
        for line in chunk.lines() {
            if let Some(json_str) = line.strip_prefix("data: ") {
                if json_str == "[DONE]" {
                    return Ok(true);
                }
                if let Ok(response) = serde_json::from_str::<ChatCompletionsResponse>(json_str) {
                    self.handle_response(&response)?;
                }
            }
        }
        Ok(false)
    }

    fn handle_response(&mut self, response: &ChatCompletionsResponse) -> Result<()> {
        if let Some(usage) = &response.usage {
            self.handler.on_usage(
                usage.prompt_tokens as u32,
                usage.completion_tokens as u32,
                usage.total_tokens as u32,
            )?;
        }

        for choice in &response.choices {
            if let Some(delta) = &choice.delta {
                self.process_delta(delta)?;
            }
            if let Some(finish_reason) = &choice.finish_reason {
                self.handler.on_finish(finish_reason)?;
            }
        }
        Ok(())
    }

    fn process_delta(&mut self, delta: &Delta) -> Result<()> {
        match delta {
            Delta::Role { role } => {
                self.handler.on_role(role)?;
            }
            Delta::Content { content } => {
                self.content_buffer.push_str(content);
                self.handler.on_content(content)?;
            }
            Delta::ToolCalls { tool_calls } => {
                self.tool_call_chunks.extend_from_slice(tool_calls);
            }
            Delta::Reasoning { reasoning_content } => {
                self.handler.on_reasoning(reasoning_content)?;
            }
            Delta::Empty {} => {}
        }
        Ok(())
    }

    fn build_tool_calls_from_chunks(&self) -> Result<Vec<ToolCall>> {
        if self.tool_call_chunks.is_empty() {
            return Ok(Vec::new());
        }

        let mut tool_calls: std::collections::HashMap<i32, ToolCall> = std::collections::HashMap::new();

        for chunk in &self.tool_call_chunks {
            let index = chunk.index.unwrap_or(0);
            
            let tool_call = tool_calls.entry(index).or_insert_with(|| ToolCall {
                id: String::new(),
                tool_type: "function".to_string(),
                function: FunctionCall {
                    name: String::new(),
                    arguments: String::new(),
                },
            });

            // Update ID if present
            if let Some(id) = &chunk.id {
                tool_call.id = id.clone();
            }

            // Update function name and accumulate arguments if present
            if let Some(function) = &chunk.function {
                if let Some(name) = &function.name {
                    tool_call.function.name = name.clone();
                }

                if let Some(args) = &function.arguments
                    && !args.trim().is_empty() {
                        tool_call.function.arguments.push_str(args);
                    }
            }
        }

        // Convert to sorted vector and validate
        let mut result: Vec<_> = tool_calls.into_values().collect();
        result.sort_by_key(|tc| tc.id.clone());

        // Validate all tool calls have complete data
        for tool_call in &result {
            if tool_call.id.is_empty() || tool_call.function.name.is_empty() {
                anyhow::bail!("Incomplete tool call: missing id or name");
            }
            
            // Ensure arguments is valid JSON, default to empty object if empty
            if tool_call.function.arguments.trim().is_empty() {
                // We can't mutate here, so we'll create a new vector with corrected data
            } else if serde_json::from_str::<serde_json::Value>(&tool_call.function.arguments).is_err() {
                anyhow::bail!(
                    "Invalid JSON in tool call arguments: {}",
                    tool_call.function.arguments
                );
            }
        }

        // Fix empty arguments
        let result: Vec<ToolCall> = result.into_iter().map(|mut tc| {
            if tc.function.arguments.trim().is_empty() {
                tc.function.arguments = "{}".to_string();
            }
            tc
        }).collect();

        Ok(result)
    }
}
