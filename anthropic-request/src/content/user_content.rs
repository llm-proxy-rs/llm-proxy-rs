use aws_sdk_bedrockruntime::types::{ContentBlock, ToolResultBlock};
use serde::{Deserialize, Serialize};

use crate::cache_control::CacheControl;
use crate::tool_result_content::ToolResultContents;

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum UserContent {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: ToolResultContents,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

impl TryFrom<&UserContent> for Vec<ContentBlock> {
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

                Ok(blocks)
            }
            UserContent::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                let tool_result_block = ToolResultBlock::builder()
                    .tool_use_id(tool_use_id)
                    .set_content(Some(content.into()))
                    .set_status(is_error.map(|e| {
                        if e {
                            aws_sdk_bedrockruntime::types::ToolResultStatus::Error
                        } else {
                            aws_sdk_bedrockruntime::types::ToolResultStatus::Success
                        }
                    }))
                    .build()?;

                Ok(vec![ContentBlock::ToolResult(tool_result_block)])
            }
        }
    }
}
