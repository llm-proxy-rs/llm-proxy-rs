use aws_sdk_bedrockruntime::types::{ContentBlock, ImageBlock, ToolResultBlock, ToolResultStatus};
use serde::{Deserialize, Serialize};

use crate::cache_control::CacheControl;
use crate::document_source::{DocumentCounter, DocumentSource};
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content: Option<ToolResultContents>,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
        tool_use_id: String,
    },
    #[serde(rename = "thinking")]
    Thinking { thinking: String, signature: String },
    #[serde(rename = "redacted_thinking")]
    RedactedThinking { data: String },
    #[serde(rename = "server_tool_result")]
    ServerToolResult {
        tool_use_id: String,
        content: serde_json::Value,
    },
}

impl UserContents {
    pub fn to_content_blocks(
        &self,
        counter: &DocumentCounter,
    ) -> anyhow::Result<Vec<ContentBlock>> {
        match self {
            UserContents::String(s) => Ok(vec![ContentBlock::Text(s.clone())]),
            UserContents::Array(arr) => {
                // Bedrock rejects more than one cache point in a single content
                // array, so only the last block that carries `cache_control`
                // emits a cache point — it caches the largest prefix.
                let last_cache_control = arr.iter().rposition(UserContent::has_cache_control);

                Ok(arr
                    .iter()
                    .enumerate()
                    .map(|(index, c)| {
                        c.to_content_blocks(counter, last_cache_control == Some(index))
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .flatten()
                    .collect())
            }
        }
    }
}

impl UserContent {
    fn has_cache_control(&self) -> bool {
        matches!(
            self,
            UserContent::Text {
                cache_control: Some(_),
                ..
            } | UserContent::ToolResult {
                cache_control: Some(_),
                ..
            }
        )
    }

    fn to_content_blocks(
        &self,
        counter: &DocumentCounter,
        emit_cache_point: bool,
    ) -> anyhow::Result<Option<Vec<ContentBlock>>> {
        match self {
            UserContent::Text {
                text,
                cache_control,
            } => {
                let mut blocks = vec![ContentBlock::Text(text.clone())];

                if emit_cache_point && let Some(cache_control) = cache_control {
                    let cache_point = cache_control.try_into()?;
                    blocks.push(ContentBlock::CachePoint(cache_point));
                }

                Ok(Some(blocks))
            }
            UserContent::Image { source } => Ok(Some(vec![ContentBlock::Image(
                ImageBlock::try_from(source)?,
            )])),
            UserContent::Document { source } => {
                let document_block = source.to_document_block(counter)?;
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
                    .set_content(Some(match content {
                        Some(c) => c.to_tool_result_content_blocks(counter)?,
                        None => vec![],
                    }))
                    .set_status(is_error.map(|is_error| {
                        if is_error {
                            ToolResultStatus::Error
                        } else {
                            ToolResultStatus::Success
                        }
                    }))
                    .build()?;

                let mut blocks = vec![ContentBlock::ToolResult(tool_result_block)];

                if emit_cache_point && let Some(cache_control) = cache_control {
                    let cache_point = cache_control.try_into()?;
                    blocks.push(ContentBlock::CachePoint(cache_point));
                }

                Ok(Some(blocks))
            }
            UserContent::Thinking { .. } => Ok(None),
            UserContent::RedactedThinking { .. } => Ok(None),
            UserContent::ServerToolResult { .. } => Ok(None),
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
        assert!(contents.to_content_blocks(&DocumentCounter::new()).is_err());
    }

    #[test]
    fn document_includes_placeholder_text_block() {
        use base64::{Engine as _, engine::general_purpose};

        let data = general_purpose::STANDARD.encode(b"%PDF-1.4");
        let json = serde_json::json!([
            {"type": "document", "source": {"type": "base64", "media_type": "application/pdf", "data": data}}
        ]);
        let contents: UserContents = serde_json::from_value(json).unwrap();
        let blocks = contents.to_content_blocks(&DocumentCounter::new()).unwrap();
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
        let blocks = contents.to_content_blocks(&DocumentCounter::new()).unwrap();
        assert_eq!(blocks.len(), 2);
        match &blocks[0] {
            ContentBlock::Text(text) => assert_eq!(text, "cached text"),
            other => panic!("expected Text, got {:?}", other),
        }
        assert!(matches!(blocks[1], ContentBlock::CachePoint(_)));
    }

    #[test]
    fn tool_result_with_missing_content_deserializes() {
        let json = serde_json::json!([
            {"type": "tool_result", "tool_use_id": "t1"}
        ]);
        let contents: UserContents = serde_json::from_value(json).unwrap();
        let blocks = contents.to_content_blocks(&DocumentCounter::new()).unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0], ContentBlock::ToolResult(_)));
    }

    #[test]
    fn document_without_title_gets_auto_name() {
        use base64::{Engine as _, engine::general_purpose};

        let data = general_purpose::STANDARD.encode(b"%PDF-1.4");
        let json = serde_json::json!([
            {"type": "document", "source": {"type": "base64", "media_type": "application/pdf", "data": data}}
        ]);
        let contents: UserContents = serde_json::from_value(json).unwrap();
        let blocks = contents.to_content_blocks(&DocumentCounter::new()).unwrap();
        assert_eq!(blocks.len(), 2);
        match &blocks[0] {
            ContentBlock::Document(doc) => assert!(doc.name().starts_with("document_")),
            other => panic!("expected Document, got {:?}", other),
        }
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
        let blocks = contents.to_content_blocks(&DocumentCounter::new()).unwrap();
        assert_eq!(blocks.len(), 2);
        match &blocks[0] {
            ContentBlock::ToolResult(result) => assert_eq!(result.tool_use_id(), "t1"),
            other => panic!("expected ToolResult, got {:?}", other),
        }
        assert!(matches!(blocks[1], ContentBlock::CachePoint(_)));
    }

    #[test]
    fn multiple_cache_controls_emit_only_last_cache_point() {
        // tool_result and a trailing text both marked — the converter must emit
        // a single cache point, for the last marked block (the text).
        let json = serde_json::json!([
            {
                "type": "tool_result",
                "tool_use_id": "t1",
                "content": "Edit applied successfully.",
                "cache_control": {"type": "ephemeral"}
            },
            {
                "type": "text",
                "text": "Create a new anchored summary...",
                "cache_control": {"type": "ephemeral"}
            }
        ]);
        let contents: UserContents = serde_json::from_value(json).unwrap();
        let blocks = contents.to_content_blocks(&DocumentCounter::new()).unwrap();

        let cache_points = blocks
            .iter()
            .filter(|b| matches!(b, ContentBlock::CachePoint(_)))
            .count();
        assert_eq!(cache_points, 1);
        // Original order is preserved: [ToolResult, Text, CachePoint].
        assert!(matches!(&blocks[0], ContentBlock::ToolResult(_)));
        assert!(matches!(&blocks[1], ContentBlock::Text(t) if t.starts_with("Create")));
        assert!(matches!(blocks[2], ContentBlock::CachePoint(_)));
    }
}
