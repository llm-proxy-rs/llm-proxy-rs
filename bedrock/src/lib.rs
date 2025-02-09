use aws_sdk_bedrockruntime::types::{ContentBlock, SystemContentBlock};
use request::{Content, Contents};

pub trait ContentsConverter {
    fn to_content_blocks(&self) -> Vec<ContentBlock>;
    fn to_system_content_blocks(&self) -> Vec<SystemContentBlock>;
}

impl ContentsConverter for Contents {
    fn to_content_blocks(&self) -> Vec<ContentBlock> {
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

    fn to_system_content_blocks(&self) -> Vec<SystemContentBlock> {
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
