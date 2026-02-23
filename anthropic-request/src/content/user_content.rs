use aws_sdk_bedrockruntime::types::{
    ContentBlock, DocumentBlock, ImageBlock, ToolResultBlock, ToolResultStatus,
};
use serde::{Deserialize, Serialize};

use crate::cache_control::CacheControl;
use crate::document_source::DocumentSource;
use crate::image_source::ImageSource;
use crate::tool_result_content::ToolResultContents;

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum UserContents {
    Array(Vec<UserContent>),
    String(String),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum UserContent {
    #[serde(rename = "text")]
    Text {
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
        text: String,
    },
    #[serde(rename = "image")]
    Image { source: ImageSource },
    #[serde(rename = "document")]
    Document { source: DocumentSource },
    #[serde(rename = "tool_result")]
    ToolResult {
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
        content: ToolResultContents,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
        tool_use_id: String,
    },
}

impl TryFrom<&UserContents> for Vec<ContentBlock> {
    type Error = anyhow::Error;

    fn try_from(contents: &UserContents) -> Result<Self, Self::Error> {
        match contents {
            UserContents::String(s) => Ok(vec![ContentBlock::Text(s.clone())]),
            UserContents::Array(arr) => Ok(arr
                .iter()
                .map(Option::<Vec<_>>::try_from)
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .flatten()
                .collect()),
        }
    }
}

impl TryFrom<&UserContent> for Option<Vec<ContentBlock>> {
    type Error = anyhow::Error;

    fn try_from(content: &UserContent) -> Result<Self, Self::Error> {
        match content {
            UserContent::Text {
                text,
                cache_control,
            } => {
                let mut blocks = vec![ContentBlock::Text(text.clone())];

                if let Some(cache_control) = cache_control {
                    let cache_point = cache_control.try_into()?;
                    blocks.push(ContentBlock::CachePoint(cache_point));
                }

                Ok(Some(blocks))
            }
            UserContent::Image { source } => Ok(Some(vec![ContentBlock::Image(
                ImageBlock::try_from(source)?,
            )])),
            UserContent::Document { source } => {
                let document_block = DocumentBlock::try_from(source)?;
                Ok(Some(vec![
                    ContentBlock::Document(document_block),
                    ContentBlock::Text(" ".into()),
                ]))
            }
            UserContent::ToolResult {
                tool_use_id,
                content,
                is_error,
                cache_control,
            } => {
                let tool_result_block = ToolResultBlock::builder()
                    .tool_use_id(tool_use_id)
                    .set_content(Some(content.try_into()?))
                    .set_status(is_error.map(|is_error| {
                        if is_error {
                            ToolResultStatus::Error
                        } else {
                            ToolResultStatus::Success
                        }
                    }))
                    .build()?;

                let mut blocks = vec![ContentBlock::ToolResult(tool_result_block)];

                if let Some(cache_control) = cache_control {
                    let cache_point = cache_control.try_into()?;
                    blocks.push(ContentBlock::CachePoint(cache_point));
                }

                Ok(Some(blocks))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_content_returns_error() {
        let json = serde_json::json!([
            {"type": "text", "text": "hello"},
            {"type": "image", "source": {"type": "base64", "media_type": "image/bmp", "data": ""}}
        ]);
        let contents: UserContents = serde_json::from_value(json).unwrap();
        assert!(Vec::<ContentBlock>::try_from(&contents).is_err());
    }

    #[test]
    fn document_includes_placeholder_text_block() {
        use base64::{Engine as _, engine::general_purpose};

        let data = general_purpose::STANDARD.encode(b"%PDF-1.4");
        let json = serde_json::json!([
            {"type": "document", "source": {"type": "base64", "media_type": "application/pdf", "data": data}}
        ]);
        let contents: UserContents = serde_json::from_value(json).unwrap();
        let blocks = Vec::<ContentBlock>::try_from(&contents).unwrap();
        assert_eq!(blocks.len(), 2);
        assert!(matches!(blocks[0], ContentBlock::Document(_)));
        assert!(matches!(blocks[1], ContentBlock::Text(_)));
    }

    #[test]
    fn text_with_cache_control() {
        let json = serde_json::json!([
            {"type": "text", "text": "cached text", "cache_control": {"type": "ephemeral"}}
        ]);
        let contents: UserContents = serde_json::from_value(json).unwrap();
        let blocks = Vec::<ContentBlock>::try_from(&contents).unwrap();
        assert_eq!(blocks.len(), 2);
        match &blocks[0] {
            ContentBlock::Text(text) => assert_eq!(text, "cached text"),
            other => panic!("expected Text, got {:?}", other),
        }
        assert!(matches!(blocks[1], ContentBlock::CachePoint(_)));
    }

    #[test]
    fn tool_result_with_cache_control() {
        let json = serde_json::json!([
            {
                "type": "tool_result",
                "tool_use_id": "t1",
                "content": "result",
                "cache_control": {"type": "ephemeral"}
            }
        ]);
        let contents: UserContents = serde_json::from_value(json).unwrap();
        let blocks = Vec::<ContentBlock>::try_from(&contents).unwrap();
        assert_eq!(blocks.len(), 2);
        match &blocks[0] {
            ContentBlock::ToolResult(result) => assert_eq!(result.tool_use_id(), "t1"),
            other => panic!("expected ToolResult, got {:?}", other),
        }
        assert!(matches!(blocks[1], ContentBlock::CachePoint(_)));
    }
}
