use aws_sdk_bedrockruntime::types::{ContentBlock as BedrockContentBlock, ReasoningContentBlock};
use common::document_to_value;

use crate::event::ContentBlock;

pub fn convert_bedrock_content_block(block: &BedrockContentBlock) -> Option<ContentBlock> {
    match block {
        BedrockContentBlock::Text(text) => {
            Some(ContentBlock::text_builder().text(text.clone()).build())
        }
        BedrockContentBlock::ToolUse(tool_use) => Some(
            ContentBlock::tool_use_builder()
                .id(tool_use.tool_use_id().to_string())
                .name(tool_use.name().to_string())
                .input(document_to_value(tool_use.input()))
                .build(),
        ),
        BedrockContentBlock::ReasoningContent(ReasoningContentBlock::ReasoningText(reasoning)) => {
            Some(
                ContentBlock::thinking_builder()
                    .thinking(reasoning.text().to_string())
                    .signature(reasoning.signature().unwrap_or_default().to_string())
                    .build(),
            )
        }
        _ => None,
    }
}

pub fn bedrock_content_blocks_to_json(
    content_blocks: &[BedrockContentBlock],
) -> Result<Vec<serde_json::Value>, serde_json::Error> {
    content_blocks
        .iter()
        .filter_map(convert_bedrock_content_block)
        .map(serde_json::to_value)
        .collect()
}
