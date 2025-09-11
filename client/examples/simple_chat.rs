use anyhow::Result;
use client::{Client, DefaultChatEventHandler, Tool, config::ClientConfig};
use request::{Message, tool::Tool as RequestTool};
use serde_json::json;
use std::sync::Arc;

pub struct TimeTool;

#[async_trait::async_trait]
impl Tool for TimeTool {
    fn definition(&self) -> RequestTool {
        RequestTool::builder()
            .function(
                request::tool::ToolFunction::builder()
                    .name("time".to_string())
                    .description(Some("Get the current time".to_string()))
                    .parameters(json!({
                        "type": "object",
                        "properties": {},
                        "required": []
                    }))
                    .build(),
            )
            .tool_type("function".to_string())
            .build()
    }

    async fn execute(&self, _args: &str) -> Result<String> {
        Ok(chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut client = Client::new(
        ClientConfig {
            base_url: "http://localhost:8080".to_string(),
            model_id: "us.anthropic.claude-sonnet-4-20250514-v1:0".to_string(),
            max_tokens: Some(1024),
            temperature: None,
            top_p: None,
        },
        Arc::new(DefaultChatEventHandler),
    );
    client.tool(Arc::new(TimeTool));

    println!("Sending request to LLM proxy...");

    client
        .message(Message::system(
            "You are a helpful assistant that can tell time.",
        ))
        .message(Message::user(
            "What time is it? Please use the available tool.",
        ));

    client.send().await?;

    client.message(Message::user("Can you tell me the time again?"));

    client.send().await?;

    Ok(())
}
