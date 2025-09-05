use anyhow::Result;
use std::io::{self, Write};

pub trait ChatEventHandler: Send + Sync {
    fn on_content(&self, content: &str) -> Result<()>;
    fn on_continuation(&self) -> Result<()>;
    fn on_finish(&self, finish_reason: &str) -> Result<()>;
    fn on_reasoning(&self, reasoning_content: &str) -> Result<()>;
    fn on_role(&self, role: &str) -> Result<()>;
    fn on_tool_call(&self, name: &str, args: &str) -> Result<()>;
    fn on_tool_error(&self, name: &str, error: &str) -> Result<()>;
    fn on_tool_result(&self, name: &str, result: &str) -> Result<()>;
    fn on_tool_start(&self, tool_count: usize) -> Result<()>;
    fn on_usage(&self, prompt_tokens: i32, completion_tokens: i32, total_tokens: i32)
    -> Result<()>;
}

pub struct DefaultChatEventHandler;

impl ChatEventHandler for DefaultChatEventHandler {
    fn on_content(&self, content: &str) -> Result<()> {
        print!("{content}");
        io::stdout().flush()?;
        Ok(())
    }

    fn on_continuation(&self) -> Result<()> {
        println!("\n[Continuing conversation with tool results...]");
        Ok(())
    }

    fn on_finish(&self, finish_reason: &str) -> Result<()> {
        println!("\n[Finished]: {finish_reason}");
        Ok(())
    }

    fn on_reasoning(&self, reasoning_content: &str) -> Result<()> {
        println!("\n[Reasoning]: {reasoning_content}");
        Ok(())
    }

    fn on_role(&self, role: &str) -> Result<()> {
        print!("\n[{role}]: ");
        io::stdout().flush()?;
        Ok(())
    }

    fn on_tool_call(&self, name: &str, args: &str) -> Result<()> {
        println!("[Tool Call]: {name}");
        println!("{args}");
        Ok(())
    }

    fn on_tool_error(&self, name: &str, error: &str) -> Result<()> {
        println!("[Tool Error] {name}: {error}");
        Ok(())
    }

    fn on_tool_result(&self, name: &str, result: &str) -> Result<()> {
        println!("[Tool Result] {name}: {result}");
        Ok(())
    }

    fn on_tool_start(&self, tool_count: usize) -> Result<()> {
        println!("\n[Executing {tool_count} tool(s)...]");
        Ok(())
    }

    fn on_usage(
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
