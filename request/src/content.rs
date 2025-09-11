use anyhow::Result;
use aws_sdk_bedrockruntime::types::{
    ContentBlock, ImageBlock, ImageFormat, ImageSource, SystemContentBlock,
};
use base64::{Engine as _, engine::general_purpose};
use serde::{
    Deserialize, Serialize,
    de::{self, SeqAccess, Visitor},
};
use std::fmt;

impl From<&ImageUrl> for Option<ImageBlock> {
    fn from(image_url: &ImageUrl) -> Self {
        let url = image_url.url.as_str();

        let (prefix, base64_data) = url.split_once(',')?;

        let format = match prefix {
            "data:image/jpeg;base64" => ImageFormat::Jpeg,
            "data:image/png;base64" => ImageFormat::Png,
            "data:image/gif;base64" => ImageFormat::Gif,
            "data:image/webp;base64" => ImageFormat::Webp,
            _ => return None,
        };

        let image_bytes = general_purpose::STANDARD.decode(base64_data).ok()?;

        ImageBlock::builder()
            .format(format)
            .source(ImageSource::Bytes(image_bytes.into()))
            .build()
            .ok()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Contents {
    Array(Vec<Content>),
    String(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageUrl {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SystemContents {
    Array(Vec<SystemContent>),
    String(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum SystemContent {
    #[serde(rename = "text")]
    Text { text: String },
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
                .filter_map(|c| match c {
                    Content::Text { text } => Some(ContentBlock::Text(text.clone())),
                    Content::ImageUrl { image_url } => {
                        Option::<ImageBlock>::from(image_url).map(ContentBlock::Image)
                    }
                })
                .collect(),
            Contents::String(s) => vec![ContentBlock::Text(s.clone())],
        }
    }
}

impl From<&SystemContents> for Vec<SystemContentBlock> {
    fn from(contents: &SystemContents) -> Self {
        match contents {
            SystemContents::Array(a) => a
                .iter()
                .map(|c| match c {
                    SystemContent::Text { text } => SystemContentBlock::Text(text.clone()),
                })
                .collect(),
            SystemContents::String(s) => vec![SystemContentBlock::Text(s.clone())],
        }
    }
}
