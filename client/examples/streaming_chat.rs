use anyhow::{Result, bail};
use chrono;
use client::{Chat, ResponseHandler, tool::Tool};
use request::{Message, tool::Tool as RequestTool};
use serde_json::json;
use std::io::{self, Write};

/// Fibonacci tool implementation
pub struct FibonacciTool;

impl Tool for FibonacciTool {
    fn definition(&self) -> RequestTool {
        RequestTool::builder()
            .name("fibonacci")
            .description("Calculate the nth fibonacci number")
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
    }

    fn execute(&self, args: Option<&str>) -> Result<String> {
        match args {
            Some(args_str) if !args_str.trim().is_empty() => {
                let args: serde_json::Value = serde_json::from_str(args_str)?;
                let n = args["n"]
                    .as_i64()
                    .ok_or_else(|| anyhow::anyhow!("Missing required parameter 'n'"))?
                    as usize;
                Ok(self.fibonacci(n).to_string())
            }
            _ => bail!("Fibonacci tool requires arguments"),
        }
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
            .name("time")
            .description("Get the current time")
            .parameters(json!({
                "type": "object",
                "properties": {},
                "required": []
            }))
            .build()
    }

    fn execute(&self, _args: Option<&str>) -> Result<String> {
        // TimeTool ignores arguments completely - no JSON parsing needed
        Ok(chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string())
    }
}

/// CLI implementation of ResponseHandler for terminal output
pub struct CliHandler;

impl ResponseHandler for CliHandler {
    fn on_role(&mut self, role: &str) -> Result<()> {
        println!("\n[{}]: ", role);
        io::stdout().flush()?;
        Ok(())
    }

    fn on_content(&mut self, content: &str) -> Result<()> {
        print!("{}", content);
        io::stdout().flush()?;
        Ok(())
    }

    fn on_reasoning(&mut self, reasoning: &str) -> Result<()> {
        println!("\n[Reasoning]: {}", reasoning);
        Ok(())
    }

    fn on_finish(&mut self, reason: &str) -> Result<()> {
        println!("\n[Finished]: {}", reason);
        Ok(())
    }

    fn on_usage(
        &mut self,
        prompt_tokens: u32,
        completion_tokens: u32,
        total_tokens: u32,
    ) -> Result<()> {
        println!(
            "\n[Usage] Prompt: {}, Completion: {}, Total: {}",
            prompt_tokens, completion_tokens, total_tokens
        );
        Ok(())
    }

    fn on_tool_start(&mut self, tool_count: usize) -> Result<()> {
        println!("\n[Executing {} tool(s)...]", tool_count);
        Ok(())
    }

    fn on_tool_call(&mut self, name: &str, args: Option<&str>) -> Result<()> {
        println!("[Tool Call]: {}", name);
        if let Some(args) = args {
            println!("{}", args);
        }
        Ok(())
    }

    fn on_tool_result(&mut self, name: &str, result: &str) -> Result<()> {
        println!("[Tool Result] {}: {}", name, result);
        Ok(())
    }

    fn on_tool_error(&mut self, name: &str, error: &str) -> Result<()> {
        println!("[Tool Error] {}: {}", name, error);
        Ok(())
    }

    fn on_continuation(&mut self) -> Result<()> {
        println!("\n[Continuing conversation with tool results...]");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create a new chat client with tools using the builder pattern
    let mut client = Chat::builder(client::config::ClientConfig {
        base_url: "http://localhost:8080".to_string(),
        model_id: "us.anthropic.claude-sonnet-4-20250514-v1:0".to_string(),
    })
    .tool(Box::new(FibonacciTool))
    .tool(Box::new(TimeTool))
    .build();

    // Add messages and send
    println!("Sending request to LLM proxy...");

    let mut handler = CliHandler;

    client
        .message(Message::system("You are a helpful assistant that can calculate fibonacci numbers and tell time."))
        .message(Message::user("Please calculate the 5th fibonacci number and tell me the current time. Use the available tools."))
        .send(&mut handler)
        .await?;

    // You can chain multiple messages using the convenience methods
    client
        .message(Message::user("What's the 10th fibonacci number?"))
        .message(Message::user("And what about the 15th?"))
        .send(&mut handler)
        .await?;

    Ok(())
}
