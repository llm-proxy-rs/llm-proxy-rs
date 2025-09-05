use anyhow::{Result, anyhow};
use request::tool::ToolCall as RequestToolCall;
use response::{Delta, ToolCall as ResponseToolCall};
use std::sync::Arc;

use super::Processor;
use crate::event::ChatEventHandler;

pub struct DeltaProcessor {
    chat_event_handler: Arc<dyn ChatEventHandler>,
    assistant_message_content: String,
    response_tool_calls: Vec<ResponseToolCall>,
}

#[async_trait::async_trait]
impl Processor<Arc<dyn ChatEventHandler>, Delta> for DeltaProcessor {
    fn new(chat_event_handler: Arc<dyn ChatEventHandler>) -> Self {
        Self {
            chat_event_handler,
            assistant_message_content: String::new(),
            response_tool_calls: Vec::new(),
        }
    }

    async fn process(&mut self, delta: Delta) -> Result<()> {
        match delta {
            Delta::Role { role } => {
                self.chat_event_handler.on_role(&role)?;
            }
            Delta::Content { content } => {
                self.assistant_message_content.push_str(&content);
                self.chat_event_handler.on_content(&content)?;
            }
            Delta::ToolCalls { tool_calls } => {
                self.response_tool_calls.extend_from_slice(&tool_calls);
            }
            Delta::Reasoning { reasoning_content } => {
                self.chat_event_handler.on_reasoning(&reasoning_content)?;
            }
            Delta::Empty {} => {}
        }
        Ok(())
    }
}

impl DeltaProcessor {
    pub fn get_assistant_message(&self) -> String {
        self.assistant_message_content.clone()
    }

    pub fn get_request_tool_calls(&self) -> Result<Vec<RequestToolCall>> {
        response_tool_calls_to_request_tool_calls(&self.response_tool_calls)
    }
}

fn new_request_tool_call(id: &str) -> RequestToolCall {
    RequestToolCall {
        id: id.to_string(),
        ..Default::default()
    }
}

fn response_tool_calls_to_request_tool_calls(
    response_tool_calls: &[ResponseToolCall],
) -> Result<Vec<RequestToolCall>> {
    let mut request_tool_calls: Vec<RequestToolCall> = Vec::new();
    let mut current_id: Option<String> = None;

    for response_tool_call in response_tool_calls {
        if let Some(id) = &response_tool_call.id
            && current_id.as_ref() != Some(id)
        {
            request_tool_calls.push(new_request_tool_call(id));
            current_id = Some(id.clone());
        }

        let tool_call = request_tool_calls
            .last_mut()
            .ok_or_else(|| anyhow!("Tool call chunk missing required ID"))?;

        if let Some(function) = &response_tool_call.function {
            if let Some(name) = &function.name {
                tool_call.function.name = name.clone();
            }

            if let Some(args) = &function.arguments {
                tool_call.function.arguments.push_str(args);
            }
        }
    }

    Ok(request_tool_calls)
}
