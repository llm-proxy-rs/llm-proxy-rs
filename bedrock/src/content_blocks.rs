use aws_sdk_bedrockruntime::types::{ContentBlock, SystemContentBlock};
use request::{Content, Contents};

pub trait ToBedrockContentBlocks<T> {
    fn to_bedrock_content_blocks(&self) -> Vec<T>;
}

impl ToBedrockContentBlocks<ContentBlock> for Contents {
    fn to_bedrock_content_blocks(&self) -> Vec<ContentBlock> {
        match self {
            Contents::Array(arr) => arr
                .iter()
                .map(|c| match c {
                    Content::Text { text } => ContentBlock::Text(text.clone()),
                })
                .collect(),
            Contents::String(s) => vec![ContentBlock::Text(s.clone())],
        }
    }
}

impl ToBedrockContentBlocks<SystemContentBlock> for Contents {
    fn to_bedrock_content_blocks(&self) -> Vec<SystemContentBlock> {
        match self {
            Contents::Array(arr) => arr
                .iter()
                .map(|c| match c {
                    Content::Text { text } => SystemContentBlock::Text(text.clone()),
                })
                .collect(),
            Contents::String(s) => vec![SystemContentBlock::Text(s.clone())],
        }
    }
}
