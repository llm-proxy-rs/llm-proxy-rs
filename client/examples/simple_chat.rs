use anyhow::{Result, bail};
use client::{Client, DefaultChatEventHandler, Tool, config::ClientConfig};
use request::{Message, tool::Tool as RequestTool};
use serde_json::json;
use std::sync::Arc;

/// Fibonacci tool implementation
pub struct FibonacciTool;

impl Tool for FibonacciTool {
    fn definition(&self) -> RequestTool {
        RequestTool::builder()
            .function(
                request::tool::ToolFunction::builder()
                    .name("fibonacci".to_string())
                    .description(Some("Calculate the nth fibonacci number".to_string()))
                    .parameters(json!({
                        "type": "object",
                        "properties": {
                            "n": {
                                "type": "integer",
                                "description": "The position in the fibonacci sequence (must be positive)"
                            }
                        },
                        "required": ["n"]
                    }))
                    .build()
            )
            .tool_type("function".to_string())
            .build()
    }

    fn execute(&self, args: &str) -> Result<String> {
        if args.trim().is_empty() {
            bail!("Fibonacci tool requires arguments");
        }

        let args: serde_json::Value = serde_json::from_str(args)?;
        let n = args["n"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter 'n'"))?
            as usize;

        Ok(self.fibonacci(n).to_string())
    }
}

impl FibonacciTool {
    /// Calculate fibonacci number recursively with upper bound
    fn fibonacci(&self, n: usize) -> u64 {
        if n > 40 {
            return 0; // Return 0 for numbers too large to compute safely
        }

        match n {
            0 => 0,
            1 => 1,
            _ => self.fibonacci(n - 1) + self.fibonacci(n - 2),
        }
    }
}

/// Time tool implementation
pub struct TimeTool;

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

    fn execute(&self, _args: &str) -> Result<String> {
        // TimeTool ignores arguments completely - no JSON parsing needed
        Ok(chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create a new chat client with tools using the builder pattern
    let mut client = Client::new(
        ClientConfig {
            base_url: "http://localhost:8080".to_string(),
            model_id: "us.anthropic.claude-sonnet-4-20250514-v1:0".to_string(),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            top_p: Some(0.9),
        },
        Arc::new(DefaultChatEventHandler),
    )
    .tool(Box::new(FibonacciTool))
    .tool(Box::new(TimeTool));

    // Add messages and send
    println!("Sending request to LLM proxy...");

    client = client
        .message(Message::system("You are a helpful assistant that can calculate fibonacci numbers and tell time."))
        .message(Message::user("Please calculate the 5th fibonacci number and tell me the current time. Use the available tools."));

    client.send().await?;

    // You can chain multiple messages using the convenience methods
    client = client
        .message(Message::user("What's the 10th fibonacci number?"))
        .message(Message::user("And what about the 15th?"));

    client.send().await?;

    Ok(())
}
