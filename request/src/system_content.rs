use aws_sdk_bedrockruntime::types::SystemContentBlock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SystemContents {
    Array(Vec<SystemContent>),
    String(String),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum SystemContent {
    #[serde(rename = "text")]
    Text { text: String },
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
