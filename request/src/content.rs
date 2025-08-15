use aws_sdk_bedrockruntime::types::{
    ContentBlock, ImageBlock, ImageFormat, ImageSource, SystemContentBlock,
};
use base64::{Engine as _, engine::general_purpose};
use serde::{
    Deserialize, Serialize,
    de::{self, SeqAccess, Visitor},
};
use std::fmt;

pub fn process_image_url(image_url: &ImageUrl) -> Result<ImageBlock, String> {
    let (format, base64_data) = match image_url.url.as_str() {
        url if url.starts_with("data:image/jpeg;base64,") => (ImageFormat::Jpeg, &url[23..]),
        url if url.starts_with("data:image/jpg;base64,") => (ImageFormat::Jpeg, &url[22..]),
        url if url.starts_with("data:image/png;base64,") => (ImageFormat::Png, &url[22..]),
        url if url.starts_with("data:image/gif;base64,") => (ImageFormat::Gif, &url[22..]),
        url if url.starts_with("data:image/webp;base64,") => (ImageFormat::Webp, &url[23..]),
        _ => {
            return Err("Invalid data URL format. Expected: data:image/{jpeg|jpg|png|gif|webp};base64,{data}".to_string());
        }
    };

    let image_bytes = general_purpose::STANDARD
        .decode(base64_data)
        .map_err(|_| "Invalid base64 image data".to_string())?;

    ImageBlock::builder()
        .format(format)
        .source(ImageSource::Bytes(image_bytes.into()))
        .build()
        .map_err(|_| "Failed to create image block".to_string())
}

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
                    Content::ImageUrl { image_url } => match process_image_url(image_url) {
                        Ok(image_block) => ContentBlock::Image(image_block),
                        Err(err) => ContentBlock::Text(format!("Error: {err}")),
                    },
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
