pub mod config;
pub mod event;
pub mod processor;
pub mod tool;

pub use processor::{
    ChatCompletionsResponseProcessor, DeltaProcessor, Processor, ResponseProcessor,
};
pub use tool::Tool;

use anyhow::Result;
use request::{ChatCompletionsRequest, Message, StreamOptions};
use std::collections::HashMap;

pub struct Client {
    pub config: config::ClientConfig,
    pub messages: Vec<Message>,
    pub tools: Option<HashMap<String, Box<dyn Tool>>>,
}

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    pub async fn send(&self) -> Result<reqwest::Response> {
        let chat_completions_request = self.get_chat_completions_request()?;
        let url = format!("{}/chat/completions", self.config.base_url);

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&chat_completions_request)
            .send()
            .await?;

        Ok(response)
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
            messages: self.messages.clone(),
            max_tokens: self.config.max_tokens,
            model: self.config.model_id.clone(),
            n: None,
            presence_penalty: None,
            stop: None,
            stream: Some(true),
            stream_options: Some(StreamOptions {
                include_usage: true,
            }),
            temperature: self.config.temperature,
            top_p: self.config.top_p,
            user: None,
            tools: self.get_tool_definitions(),
            tool_choice: None,
            reasoning_effort: None,
        })
    }
}

#[derive(Default)]
pub struct ClientBuilder {
    pub config: config::ClientConfig,
    pub messages: Vec<Message>,
    pub tools: Option<HashMap<String, Box<dyn Tool>>>,
}

impl ClientBuilder {
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

    pub fn build(self) -> Client {
        Client {
            config: self.config,
            messages: self.messages,
            tools: self.tools,
        }
    }
}
