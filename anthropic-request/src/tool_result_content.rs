use aws_sdk_bedrockruntime::types::{DocumentBlock, ImageBlock, ToolResultContentBlock};
use serde::{Deserialize, Serialize};

use crate::document_source::DocumentSource;
use crate::image_source::ImageSource;

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
    #[serde(rename = "image")]
    Image { source: ImageSource },
    #[serde(rename = "document")]
    Document { source: DocumentSource },
}

impl From<&ToolResultContent> for Option<ToolResultContentBlock> {
    fn from(content: &ToolResultContent) -> Self {
        match content {
            ToolResultContent::Text { text } => Some(ToolResultContentBlock::Text(text.clone())),
            ToolResultContent::Image { source } => {
                Option::<ImageBlock>::from(source).map(ToolResultContentBlock::Image)
            }
            ToolResultContent::Document { source } => {
                Option::<DocumentBlock>::from(source).map(ToolResultContentBlock::Document)
            }
        }
    }
}

impl From<&ToolResultContents> for Vec<ToolResultContentBlock> {
    fn from(contents: &ToolResultContents) -> Self {
        match contents {
            ToolResultContents::String(s) => vec![ToolResultContentBlock::Text(s.clone())],
            ToolResultContents::Array(a) => a
                .iter()
                .filter_map(Option::<ToolResultContentBlock>::from)
                .collect(),
        }
    }
}
