use aws_sdk_bedrockruntime::types::{ContentBlock, SystemContentBlock};
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
                })
                .collect(),
            Contents::String(s) => vec![SystemContentBlock::Text(s.clone())],
        }
    }
}
