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
