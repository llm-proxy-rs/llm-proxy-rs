use aws_sdk_bedrockruntime::types::{CachePointBlock, SystemContentBlock};
use serde::{Deserialize, Serialize};

use crate::cache_control::CacheControl;

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Systems {
    String(String),
    Array(Vec<System>),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum System {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
}

impl TryFrom<&System> for Vec<SystemContentBlock> {
    type Error = anyhow::Error;

    fn try_from(system: &System) -> Result<Self, Self::Error> {
        match system {
            System::Text {
                text,
                cache_control,
            } => {
                let mut system_content_blocks = vec![SystemContentBlock::Text(text.clone())];

                if let Some(cache_control) = cache_control {
                    let cache_point = CachePointBlock::try_from(cache_control)?;
                    system_content_blocks.push(SystemContentBlock::CachePoint(cache_point));
                }

                Ok(system_content_blocks)
            }
        }
    }
}

impl TryFrom<&Systems> for Vec<SystemContentBlock> {
    type Error = anyhow::Error;

    fn try_from(systems: &Systems) -> Result<Self, Self::Error> {
        match systems {
            Systems::String(s) => Ok(vec![SystemContentBlock::Text(s.clone())]),
            Systems::Array(a) => Ok(a
                .iter()
                .map(Vec::<SystemContentBlock>::try_from)
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_text_to_content_blocks() {
        let system = System::Text {
            text: "You are helpful".to_string(),
            cache_control: None,
        };
        let blocks = Vec::<SystemContentBlock>::try_from(&system).unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], SystemContentBlock::Text(t) if t == "You are helpful"));
    }

    #[test]
    fn system_text_with_cache_control() {
        let system = System::Text {
            text: "You are helpful".to_string(),
            cache_control: Some(CacheControl {
                cache_control_type: "ephemeral".to_string(),
            }),
        };
        let blocks = Vec::<SystemContentBlock>::try_from(&system).unwrap();
        assert_eq!(blocks.len(), 2);
        assert!(matches!(&blocks[0], SystemContentBlock::Text(t) if t == "You are helpful"));
        assert!(matches!(blocks[1], SystemContentBlock::CachePoint(_)));
    }

    #[test]
    fn systems_string_to_content_blocks() {
        let systems = Systems::String("system prompt".to_string());
        let blocks = Vec::<SystemContentBlock>::try_from(&systems).unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], SystemContentBlock::Text(t) if t == "system prompt"));
    }

    #[test]
    fn systems_array_with_cache_to_content_blocks() {
        let systems = Systems::Array(vec![
            System::Text {
                text: "first".to_string(),
                cache_control: Some(CacheControl {
                    cache_control_type: "ephemeral".to_string(),
                }),
            },
            System::Text {
                text: "second".to_string(),
                cache_control: None,
            },
        ]);
        let blocks = Vec::<SystemContentBlock>::try_from(&systems).unwrap();
        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[0], SystemContentBlock::Text(t) if t == "first"));
        assert!(matches!(blocks[1], SystemContentBlock::CachePoint(_)));
        assert!(matches!(&blocks[2], SystemContentBlock::Text(t) if t == "second"));
    }
}
