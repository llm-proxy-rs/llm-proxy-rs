use aws_sdk_bedrockruntime::types::{
    ContentBlock, ImageBlock, ImageFormat, ImageSource, SystemContentBlock,
};
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
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ImageUrl {
    pub url: String, // This should be a data URL like "data:image/jpeg;base64,..."
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
                    Content::ImageUrl { image_url } => {
                        // Parse data URL format: data:image/jpeg;base64,<base64_data>
                        let (format, base64_data) = if image_url.url.starts_with("data:image/") {
                            let parts: Vec<&str> = image_url.url.split(',').collect();
                            if parts.len() == 2 {
                                let header = parts[0];
                                let data = parts[1];

                                // Extract format from header like "data:image/jpeg;base64"
                                let format = if header.contains("jpeg") || header.contains("jpg") {
                                    ImageFormat::Jpeg
                                } else if header.contains("png") {
                                    ImageFormat::Png
                                } else if header.contains("gif") {
                                    ImageFormat::Gif
                                } else if header.contains("webp") {
                                    ImageFormat::Webp
                                } else {
                                    ImageFormat::Jpeg // default
                                };

                                (format, data.to_string())
                            } else {
                                (ImageFormat::Jpeg, image_url.url.clone())
                            }
                        } else {
                            // Assume it's raw base64 data
                            (ImageFormat::Jpeg, image_url.url.clone())
                        };

                        // Decode base64 data
                        let image_bytes = match general_purpose::STANDARD.decode(&base64_data) {
                            Ok(bytes) => bytes,
                            Err(_) => {
                                return ContentBlock::Text(
                                    "Error: Invalid base64 image data".to_string(),
                                );
                            }
                        };

                        let image_block = match ImageBlock::builder()
                            .format(format)
                            .source(ImageSource::Bytes(image_bytes.into()))
                            .build()
                        {
                            Ok(block) => block,
                            Err(_) => {
                                return ContentBlock::Text(
                                    "Error: Failed to create image block".to_string(),
                                );
                            }
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
                    Content::ImageUrl { .. } => SystemContentBlock::Text(
                        "Error: Images not supported in system messages".to_string(),
                    ),
                })
                .collect(),
            Contents::String(s) => vec![SystemContentBlock::Text(s.clone())],
        }
    }
}
