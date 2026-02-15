use aws_sdk_bedrockruntime::types::ToolResultContentBlock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ToolResultContents {
    String(String),
    Array(Vec<ToolResultContent>),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ToolResultContent {
    #[serde(rename = "text")]
    Text { text: String },
}

impl From<&ToolResultContent> for ToolResultContentBlock {
    fn from(content: &ToolResultContent) -> Self {
        match content {
            ToolResultContent::Text { text } => ToolResultContentBlock::Text(text.clone()),
        }
    }
}

impl From<&ToolResultContents> for Vec<ToolResultContentBlock> {
    fn from(contents: &ToolResultContents) -> Self {
        match contents {
            ToolResultContents::String(s) => vec![ToolResultContentBlock::Text(s.clone())],
            ToolResultContents::Array(a) => a.iter().map(ToolResultContentBlock::from).collect(),
        }
    }
}
