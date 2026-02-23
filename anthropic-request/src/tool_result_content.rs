use aws_sdk_bedrockruntime::types::{ImageBlock, ToolResultContentBlock};
use serde::{Deserialize, Serialize};

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
}

impl TryFrom<&ToolResultContent> for ToolResultContentBlock {
    type Error = anyhow::Error;

    fn try_from(content: &ToolResultContent) -> Result<Self, Self::Error> {
        match content {
            ToolResultContent::Text { text } => Ok(ToolResultContentBlock::Text(text.clone())),
            ToolResultContent::Image { source } => {
                Ok(ToolResultContentBlock::Image(ImageBlock::try_from(source)?))
            }
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
                .map(ToolResultContentBlock::try_from)
                .collect::<Result<Vec<_>, _>>(),
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
}
