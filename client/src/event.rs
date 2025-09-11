use anyhow::Result;
use async_trait::async_trait;
use std::io::{self, Write};

#[async_trait]
pub trait ChatEventHandler: Send + Sync {
    async fn on_content(&self, content: &str) -> Result<()>;
    async fn on_continuation(&self) -> Result<()>;
    async fn on_finish(&self, finish_reason: &str) -> Result<()>;
    async fn on_reasoning(&self, reasoning_content: &str) -> Result<()>;
    async fn on_role(&self, role: &str) -> Result<()>;
    async fn on_tool_call(&self, name: &str, args: &str) -> Result<()>;
    async fn on_tool_error(&self, name: &str, error: &str) -> Result<()>;
    async fn on_tool_result(&self, name: &str, result: &str) -> Result<()>;
    async fn on_tool_start(&self, tool_count: usize) -> Result<()>;
    async fn on_usage(
        &self,
        prompt_tokens: i32,
        completion_tokens: i32,
        total_tokens: i32,
    ) -> Result<()>;
}

pub struct DefaultChatEventHandler;

#[async_trait]
impl ChatEventHandler for DefaultChatEventHandler {
    async fn on_content(&self, content: &str) -> Result<()> {
        print!("{content}");
        io::stdout().flush()?;
        Ok(())
    }

    async fn on_continuation(&self) -> Result<()> {
        println!("\n[Continuing conversation with tool results...]");
        Ok(())
    }

    async fn on_finish(&self, finish_reason: &str) -> Result<()> {
        println!("\n[Finished]: {finish_reason}");
        Ok(())
    }

    async fn on_reasoning(&self, reasoning_content: &str) -> Result<()> {
        println!("\n[Reasoning]: {reasoning_content}");
        Ok(())
    }

    async fn on_role(&self, role: &str) -> Result<()> {
        print!("\n[{role}]: ");
        io::stdout().flush()?;
        Ok(())
    }

    async fn on_tool_call(&self, name: &str, args: &str) -> Result<()> {
        println!("[Tool Call]: {name}");
        println!("{args}");
        Ok(())
    }

    async fn on_tool_error(&self, name: &str, error: &str) -> Result<()> {
        println!("[Tool Error] {name}: {error}");
        Ok(())
    }

    async fn on_tool_result(&self, name: &str, result: &str) -> Result<()> {
        println!("[Tool Result] {name}: {result}");
        Ok(())
    }

    async fn on_tool_start(&self, tool_count: usize) -> Result<()> {
        println!("\n[Executing {tool_count} tool(s)...]");
        Ok(())
    }

    async fn on_usage(
        &self,
        prompt_tokens: i32,
        completion_tokens: i32,
        total_tokens: i32,
    ) -> Result<()> {
        println!(
            "\n[Usage] Prompt: {prompt_tokens}, Completion: {completion_tokens}, Total: {total_tokens}"
        );
        Ok(())
    }
}
