use aws_sdk_bedrockruntime::types::{ContentBlock, SystemContentBlock};
use request::{Content, Contents};

pub trait IntoContentBlocks {
    fn into_content_blocks(self) -> Vec<ContentBlock>;
}

pub trait IntoSystemContentBlocks {
    fn into_system_content_blocks(self) -> Vec<SystemContentBlock>;
}

impl IntoContentBlocks for Contents {
    fn into_content_blocks(self) -> Vec<ContentBlock> {
        match self {
            Contents::Array(arr) => arr
                .into_iter()
                .map(|c| match c {
                    Content::Text { text } => ContentBlock::Text(text),
                })
                .collect(),
            Contents::String(s) => vec![ContentBlock::Text(s)],
        }
    }
}

impl IntoSystemContentBlocks for Contents {
    fn into_system_content_blocks(self) -> Vec<SystemContentBlock> {
        match self {
            Contents::Array(arr) => arr
                .into_iter()
                .map(|c| match c {
                    Content::Text { text } => SystemContentBlock::Text(text),
                })
                .collect(),
            Contents::String(s) => vec![SystemContentBlock::Text(s)],
        }
    }
}
