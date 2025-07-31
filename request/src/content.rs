use aws_sdk_bedrockruntime::types::{ContentBlock, SystemContentBlock, ImageBlock, ImageFormat, ImageSource};
use base64::{Engine as _, engine::general_purpose};
use serde::{
    Deserialize, Serialize,
    de::{self, SeqAccess, Visitor},
};
use std::fmt;

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Contents {
    Array(Vec<Content>),
    String(String),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { 
        image: ImageContent 
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ImageContent {
    pub format: String,
    pub data: String, // base64 encoded image data
}

impl<'de> Visitor<'de> for Contents {
    type Value = Contents;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("string or array")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Contents::String(value.to_string()))
    }

    fn visit_seq<S>(self, seq: S) -> Result<Self::Value, S::Error>
    where
        S: SeqAccess<'de>,
    {
        let content_vec: Vec<Content> =
            Deserialize::deserialize(de::value::SeqAccessDeserializer::new(seq))?;
        Ok(Contents::Array(content_vec))
    }
}

impl From<&Contents> for Vec<ContentBlock> {
    fn from(contents: &Contents) -> Self {
        match contents {
            Contents::Array(a) => a
                .iter()
                .map(|c| match c {
                    Content::Text { text } => ContentBlock::Text(text.clone()),
                    Content::Image { image } => {
                        let format = match image.format.to_lowercase().as_str() {
                            "jpeg" | "jpg" => ImageFormat::Jpeg,
                            "png" => ImageFormat::Png,
                            "gif" => ImageFormat::Gif,
                            "webp" => ImageFormat::Webp,
                            _ => ImageFormat::Png, // default to PNG if unknown
                        };
                        
                        // Decode base64 data using new API
                        let image_bytes = match general_purpose::STANDARD.decode(&image.data) {
                            Ok(bytes) => bytes,
                            Err(_) => return ContentBlock::Text("Error: Invalid base64 image data".to_string()),
                        };
                        
                        let image_block = match ImageBlock::builder()
                            .format(format)
                            .source(ImageSource::Bytes(image_bytes.into()))
                            .build()
                        {
                            Ok(block) => block,
                            Err(_) => return ContentBlock::Text("Error: Failed to create image block".to_string()),
                        };
                        
                        ContentBlock::Image(image_block)
                    }
                })
                .collect(),
            Contents::String(s) => vec![ContentBlock::Text(s.clone())],
        }
    }
}

impl From<&Contents> for Vec<SystemContentBlock> {
    fn from(contents: &Contents) -> Self {
        match contents {
            Contents::Array(a) => a
                .iter()
                .map(|c| match c {
                    Content::Text { text } => SystemContentBlock::Text(text.clone()),
                    // System content blocks don't support images in AWS Bedrock
                    Content::Image { .. } => SystemContentBlock::Text("Error: Images not supported in system messages".to_string()),
                })
                .collect(),
            Contents::String(s) => vec![SystemContentBlock::Text(s.clone())],
        }
    }
}
