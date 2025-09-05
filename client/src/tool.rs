use anyhow::Result;
use request::tool::Tool as RequestTool;

pub trait Tool {
    fn definition(&self) -> RequestTool;
    fn execute(&self, args: Option<&str>) -> Result<String>;
}
