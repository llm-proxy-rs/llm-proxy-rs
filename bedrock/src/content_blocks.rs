use aws_sdk_bedrockruntime::types::{ContentBlock, SystemContentBlock};
use request::{Content, Contents};

pub fn contents_to_bedrock_content_block(contents: &Contents) -> Vec<ContentBlock> {
    match contents {
        Contents::Array(arr) => arr
            .iter()
            .map(|c| match c {
                Content::Text { text } => ContentBlock::Text(text.clone()),
            })
            .collect(),
        Contents::String(s) => vec![ContentBlock::Text(s.clone())],
    }
}

pub fn contents_to_bedrock_system_content_block(contents: &Contents) -> Vec<SystemContentBlock> {
    match contents {
        Contents::Array(arr) => arr
            .iter()
            .map(|c| match c {
                Content::Text { text } => SystemContentBlock::Text(text.clone()),
            })
            .collect(),
        Contents::String(s) => vec![SystemContentBlock::Text(s.clone())],
    }
}
