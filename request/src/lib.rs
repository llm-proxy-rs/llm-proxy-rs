use aws_sdk_bedrockruntime::types::{ContentBlock, ConversationRole, SystemContentBlock};
use serde::{
    Deserialize, Serialize,
    de::{self, SeqAccess, Visitor},
};
use std::{collections::HashMap, fmt};

#[derive(Debug, Deserialize, Serialize)]
pub struct ChatCompletionsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StreamOptions {
    pub include_usage: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Message {
    #[serde(rename = "content")]
    pub contents: Contents,
    pub role: Role,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Assistant,
    System,
    User,
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
            Contents::Array(arr) => arr
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

impl From<&Role> for ConversationRole {
    fn from(role: &Role) -> Self {
        match role {
            Role::Assistant => ConversationRole::Assistant,
            Role::User => ConversationRole::User,
            Role::System => unreachable!(),
        }
    }
}

impl TryFrom<&Message> for aws_sdk_bedrockruntime::types::Message {
    type Error = aws_sdk_bedrockruntime::error::BuildError;

    fn try_from(message: &Message) -> Result<Self, Self::Error> {
        aws_sdk_bedrockruntime::types::Message::builder()
            .set_role(Some((&message.role).into()))
            .set_content(Some((&message.contents).into()))
            .build()
    }
}
