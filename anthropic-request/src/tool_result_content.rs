use aws_sdk_bedrockruntime::types::{DocumentBlock, ImageBlock, ToolResultContentBlock};
use common::value_to_document;
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
    #[serde(rename = "document")]
    Document { source: DocumentSource },
    #[serde(rename = "image")]
    Image { source: ImageSource },
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_reference")]
    ToolReference { tool_name: String },
    #[serde(other)]
    Unknown,
}

impl TryFrom<&ToolResultContent> for Option<ToolResultContentBlock> {
    type Error = anyhow::Error;

    fn try_from(content: &ToolResultContent) -> Result<Self, Self::Error> {
        match content {
            ToolResultContent::Document { source } => Ok(Some(ToolResultContentBlock::Document(
                DocumentBlock::try_from(source)?,
            ))),
            ToolResultContent::Image { source } => Ok(Some(ToolResultContentBlock::Image(
                ImageBlock::try_from(source)?,
            ))),
            ToolResultContent::Text { text } => {
                Ok(Some(ToolResultContentBlock::Text(text.clone())))
            }
            ToolResultContent::ToolReference { .. } => {
                let value = serde_json::to_value(content)?;
                Ok(Some(ToolResultContentBlock::Json(value_to_document(
                    &value,
                ))))
            }
            ToolResultContent::Unknown => Ok(None),
        }
    }
}

impl TryFrom<&ToolResultContents> for Vec<ToolResultContentBlock> {
    type Error = anyhow::Error;

    fn try_from(contents: &ToolResultContents) -> Result<Self, Self::Error> {
        match contents {
            ToolResultContents::String(s) => Ok(vec![ToolResultContentBlock::Text(s.clone())]),
            ToolResultContents::Array(a) => a
                .iter()
                .map(Option::<ToolResultContentBlock>::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map(|v| v.into_iter().flatten().collect()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_result_with_image_deserializes() {
        let json = serde_json::json!([
            {
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": "image/png",
                    "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
                }
            }
        ]);
        let contents: ToolResultContents = serde_json::from_value(json).unwrap();
        let blocks = Vec::<ToolResultContentBlock>::try_from(&contents).unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0], ToolResultContentBlock::Image(_)));
    }

    #[test]
    fn tool_result_with_mixed_content_deserializes() {
        let json = serde_json::json!([
            {"type": "text", "text": "Here is the screenshot:"},
            {
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": "image/png",
                    "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
                }
            }
        ]);
        let contents: ToolResultContents = serde_json::from_value(json).unwrap();
        let blocks = Vec::<ToolResultContentBlock>::try_from(&contents).unwrap();
        assert_eq!(blocks.len(), 2);
        assert!(matches!(blocks[0], ToolResultContentBlock::Text(_)));
        assert!(matches!(blocks[1], ToolResultContentBlock::Image(_)));
    }

    #[test]
    fn tool_result_string_content_deserializes() {
        let json = serde_json::json!("plain text result");
        let contents: ToolResultContents = serde_json::from_value(json).unwrap();
        let blocks = Vec::<ToolResultContentBlock>::try_from(&contents).unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0], ToolResultContentBlock::Text(_)));
    }

    #[test]
    fn tool_result_with_tool_reference_becomes_json() {
        let json = serde_json::json!([
            {"type": "tool_reference", "tool_name": "AskUserQuestion"}
        ]);
        let contents: ToolResultContents = serde_json::from_value(json).unwrap();
        let blocks = Vec::<ToolResultContentBlock>::try_from(&contents).unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0], ToolResultContentBlock::Json(_)));
    }

    #[test]
    fn tool_result_with_document_deserializes() {
        use base64::{Engine as _, engine::general_purpose};

        let data = general_purpose::STANDARD.encode(b"%PDF-1.4");
        let json = serde_json::json!([
            {
                "type": "document",
                "source": {
                    "type": "base64",
                    "media_type": "application/pdf",
                    "data": data
                }
            }
        ]);
        let contents: ToolResultContents = serde_json::from_value(json).unwrap();
        let blocks = Vec::<ToolResultContentBlock>::try_from(&contents).unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0], ToolResultContentBlock::Document(_)));
    }

    #[test]
    fn tool_result_with_unknown_type_mixed_keeps_known() {
        let json = serde_json::json!([
            {"type": "text", "text": "hello"},
            {"type": "tool_reference", "tool_name": "AskUserQuestion"}
        ]);
        let contents: ToolResultContents = serde_json::from_value(json).unwrap();
        let blocks = Vec::<ToolResultContentBlock>::try_from(&contents).unwrap();
        assert_eq!(blocks.len(), 2);
        assert!(matches!(blocks[0], ToolResultContentBlock::Text(_)));
        assert!(matches!(blocks[1], ToolResultContentBlock::Json(_)));
    }
}
