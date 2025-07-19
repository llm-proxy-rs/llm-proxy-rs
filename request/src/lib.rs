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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OpenAITool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<OpenAIToolChoice>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StreamOptions {
    pub include_usage: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OpenAITool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenAIToolFunction,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OpenAIToolFunction {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum OpenAIToolChoice {
    String(String),
    Object {
        #[serde(rename = "type")]
        tool_type: String,
        function: OpenAIToolChoiceFunction,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OpenAIToolChoiceFunction {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Message {
    #[serde(rename = "content")]
    pub contents: Contents,
    pub role: Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OpenAIToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenAIFunctionCall,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OpenAIFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Assistant,
    System,
    User,
    Tool,
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

impl TryFrom<&Message> for aws_sdk_bedrockruntime::types::Message {
    type Error = anyhow::Error;

    fn try_from(message: &Message) -> Result<Self, Self::Error> {
        let content_blocks: Vec<ContentBlock> = (&message.contents).into();

        match message.role {
            Role::Assistant => aws_sdk_bedrockruntime::types::Message::builder()
                .role(ConversationRole::Assistant)
                .set_content(Some(content_blocks))
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to build Assistant message: {e}")),
            Role::User => aws_sdk_bedrockruntime::types::Message::builder()
                .role(ConversationRole::User)
                .set_content(Some(content_blocks))
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to build User message: {e}")),
            _ => anyhow::bail!(
                "Only User and Assistant roles are supported in messages, found: {:?}",
                message.role
            ),
        }
    }
}
