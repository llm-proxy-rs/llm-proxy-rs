use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub cache_control_type: String,
}

impl TryFrom<&CacheControl> for aws_sdk_bedrockruntime::types::CachePointBlock {
    type Error = anyhow::Error;

    fn try_from(_: &CacheControl) -> Result<Self, Self::Error> {
        Ok(aws_sdk_bedrockruntime::types::CachePointBlock::builder()
            .r#type(aws_sdk_bedrockruntime::types::CachePointType::Default)
            .build()?)
    }
}
