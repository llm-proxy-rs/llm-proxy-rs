use anyhow::Result;
use std::io::{self, Write};

pub trait ChatEventHandler: Send + Sync {
    fn on_role(&self, role: &str) -> Result<()>;
    fn on_content(&self, content: &str) -> Result<()>;
    fn on_reasoning(&self, reasoning_content: &str) -> Result<()>;
    fn on_usage(&self, prompt_tokens: i32, completion_tokens: i32, total_tokens: i32)
    -> Result<()>;
    fn on_finish(&self, finish_reason: &str) -> Result<()>;
    fn on_tool_start(&self, tool_count: usize) -> Result<()>;
    fn on_tool_call(&self, name: &str, args: Option<&str>) -> Result<()>;
    fn on_tool_result(&self, name: &str, result: &str) -> Result<()>;
    fn on_tool_error(&self, name: &str, error: &str) -> Result<()>;
    fn on_continuation(&self) -> Result<()>;
}

/// Default console-based event handler with rich output
pub struct DefaultChatEventHandler;

impl ChatEventHandler for DefaultChatEventHandler {
    fn on_role(&self, role: &str) -> Result<()> {
        print!("\n[{}]: ", role);
        io::stdout().flush()?;
        Ok(())
    }

    fn on_content(&self, content: &str) -> Result<()> {
        print!("{}", content);
        io::stdout().flush()?;
        Ok(())
    }

    fn on_reasoning(&self, reasoning_content: &str) -> Result<()> {
        println!("\n[Reasoning]: {}", reasoning_content);
        Ok(())
    }

    fn on_finish(&self, finish_reason: &str) -> Result<()> {
        println!("\n[Finished]: {}", finish_reason);
        Ok(())
    }

    fn on_usage(
        &self,
        prompt_tokens: i32,
        completion_tokens: i32,
        total_tokens: i32,
    ) -> Result<()> {
        println!(
            "\n[Usage] Prompt: {}, Completion: {}, Total: {}",
            prompt_tokens, completion_tokens, total_tokens
        );
        Ok(())
    }

    fn on_tool_start(&self, tool_count: usize) -> Result<()> {
        println!("\n[Executing {} tool(s)...]", tool_count);
        Ok(())
    }

    fn on_tool_call(&self, name: &str, args: Option<&str>) -> Result<()> {
        println!("[Tool Call]: {}", name);
        if let Some(args) = args {
            println!("{}", args);
        }
        Ok(())
    }

    fn on_tool_result(&self, name: &str, result: &str) -> Result<()> {
        println!("[Tool Result] {}: {}", name, result);
        Ok(())
    }

    fn on_tool_error(&self, name: &str, error: &str) -> Result<()> {
        println!("[Tool Error] {}: {}", name, error);
        Ok(())
    }

    fn on_continuation(&self) -> Result<()> {
        println!("\n[Continuing conversation with tool results...]");
        Ok(())
    }
}
