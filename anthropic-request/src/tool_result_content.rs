use aws_sdk_bedrockruntime::types::{
    ImageBlock, ImageFormat, ImageSource as BedrockImageSource,
    ToolResultContentBlock as BedrockToolResultContentBlock,
};
use aws_smithy_types::Blob;
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum ToolResultContent {
    String(String),
    Blocks(Vec<ToolResultContentBlock>),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum ToolResultContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageSource },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ImageSource {
    pub data: String,
    pub media_type: String,
    #[serde(rename = "type")]
    pub source_type: String,
}

impl From<&ImageSource> for Option<ImageBlock> {
    fn from(source: &ImageSource) -> Self {
        if source.source_type != "base64" {
            return None;
        }

        let format = match source.media_type.as_str() {
            "image/jpeg" => ImageFormat::Jpeg,
            "image/png" => ImageFormat::Png,
            "image/gif" => ImageFormat::Gif,
            "image/webp" => ImageFormat::Webp,
            _ => return None,
        };

        let image_bytes = general_purpose::STANDARD.decode(&source.data).ok()?;

        ImageBlock::builder()
            .format(format)
            .source(BedrockImageSource::Bytes(Blob::new(image_bytes)))
            .build()
            .ok()
    }
}

impl From<&ToolResultContent> for Vec<BedrockToolResultContentBlock> {
    fn from(content: &ToolResultContent) -> Self {
        match content {
            ToolResultContent::String(s) => {
                vec![BedrockToolResultContentBlock::Text(s.clone())]
            }
            ToolResultContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ToolResultContentBlock::Text { text } => {
                        Some(BedrockToolResultContentBlock::Text(text.clone()))
                    }
                    ToolResultContentBlock::Image { source } => {
                        Option::<ImageBlock>::from(source).map(BedrockToolResultContentBlock::Image)
                    }
                })
                .collect(),
        }
    }
}
