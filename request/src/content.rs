use aws_sdk_bedrockruntime::types::{ContentBlock, ImageBlock};
use serde::{
    Deserialize, Serialize,
    de::{self, SeqAccess, Visitor},
};
use std::fmt;

use crate::image_url::ImageUrl;

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

impl TryFrom<&Contents> for Vec<ContentBlock> {
    type Error = anyhow::Error;

    fn try_from(contents: &Contents) -> Result<Self, Self::Error> {
        match contents {
            Contents::Array(a) => a
                .iter()
                .map(|c| match c {
                    Content::Text { text } => Ok(Some(ContentBlock::Text(text.clone()))),
                    Content::ImageUrl { image_url } => {
                        Ok(Some(ContentBlock::Image(ImageBlock::try_from(image_url)?)))
                    }
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|v| v.into_iter().flatten().collect()),
            Contents::String(s) => Ok(vec![ContentBlock::Text(s.clone())]),
        }
    }
}
