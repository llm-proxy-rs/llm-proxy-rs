use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub cache_control_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
}

impl TryFrom<&CacheControl> for aws_sdk_bedrockruntime::types::CachePointBlock {
    type Error = anyhow::Error;

    fn try_from(cache_control: &CacheControl) -> Result<Self, Self::Error> {
        let ttl = cache_control
            .ttl
            .as_deref()
            .map(aws_sdk_bedrockruntime::types::CacheTtl::from);

        Ok(aws_sdk_bedrockruntime::types::CachePointBlock::builder()
            .r#type(aws_sdk_bedrockruntime::types::CachePointType::Default)
            .set_ttl(ttl)
            .build()?)
    }
}

#[cfg(test)]
mod tests {
    use aws_sdk_bedrockruntime::types::{CachePointBlock, CacheTtl};

    use super::*;

    #[test]
    fn cache_control_without_ttl_produces_no_bedrock_ttl() {
        let cache_control = CacheControl {
            cache_control_type: "ephemeral".to_string(),
            ttl: None,
        };
        let cache_point = CachePointBlock::try_from(&cache_control).unwrap();
        assert!(cache_point.ttl().is_none());
    }

    #[test]
    fn cache_control_with_five_minute_ttl_maps_to_bedrock() {
        let cache_control = CacheControl {
            cache_control_type: "ephemeral".to_string(),
            ttl: Some("5m".to_string()),
        };
        let cache_point = CachePointBlock::try_from(&cache_control).unwrap();
        assert_eq!(cache_point.ttl(), Some(&CacheTtl::FiveMinutes));
    }

    #[test]
    fn cache_control_with_one_hour_ttl_maps_to_bedrock() {
        let cache_control = CacheControl {
            cache_control_type: "ephemeral".to_string(),
            ttl: Some("1h".to_string()),
        };
        let cache_point = CachePointBlock::try_from(&cache_control).unwrap();
        assert_eq!(cache_point.ttl(), Some(&CacheTtl::OneHour));
    }
}
