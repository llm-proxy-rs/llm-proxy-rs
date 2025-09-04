use anyhow::Result;

pub trait ChatEventHandler: Send + Sync {
    fn on_role(&self, role: &str) -> Result<()>;
    fn on_content(&self, content: &str) -> Result<()>;
    fn on_reasoning(&self, reasoning_content: &str) -> Result<()>;
    fn on_usage(&self, prompt_tokens: i32, completion_tokens: i32, total_tokens: i32)
    -> Result<()>;
    fn on_finish(&self, finish_reason: &str) -> Result<()>;
}
