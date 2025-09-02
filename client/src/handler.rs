use anyhow::Result;

pub trait ChatEventHandler {
    fn on_content(&mut self, content: &str) -> Result<()>;

    fn on_continuation(&mut self) -> Result<()>;

    fn on_finish(&mut self, reason: &str) -> Result<()>;

    fn on_reasoning(&mut self, reasoning: &str) -> Result<()>;

    fn on_role(&mut self, role: &str) -> Result<()>;

    fn on_tool_call(&mut self, name: &str, args: Option<&str>) -> Result<()>;

    fn on_tool_error(&mut self, name: &str, error: &str) -> Result<()>;

    fn on_tool_result(&mut self, name: &str, result: &str) -> Result<()>;

    fn on_tool_start(&mut self, tool_count: usize) -> Result<()>;

    fn on_usage(
        &mut self,
        prompt_tokens: u32,
        completion_tokens: u32,
        total_tokens: u32,
    ) -> Result<()>;
}
