use anyhow::{Result, bail};
use aws_sdk_bedrockruntime::types::{
    JsonSchemaDefinition, OutputConfig as BedrockOutputConfig, OutputFormat as BedrockOutputFormat,
    OutputFormatStructure, OutputFormatType,
};
use aws_smithy_types::Document;
use serde::{Deserialize, Serialize};

use crate::Thinking;

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum OutputConfig {
    Format { format: OutputConfigFormat },
    Effort { effort: String },
    Other(serde_json::Value),
}

fn effort_document(effort: &str) -> Document {
    Document::Object(
        [
            (
                "output_config".to_string(),
                Document::Object(
                    [("effort".to_string(), Document::String(effort.to_string()))]
                        .into_iter()
                        .collect(),
                ),
            ),
            (
                "anthropic_beta".to_string(),
                Document::Array(vec![Document::String("effort-2025-11-24".to_string())]),
            ),
        ]
        .into_iter()
        .collect(),
    )
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OutputConfigFormat {
    #[serde(rename = "type")]
    pub format_type: String,
    pub schema: serde_json::Value,
}

impl TryFrom<&OutputConfigFormat> for BedrockOutputConfig {
    type Error = anyhow::Error;

    fn try_from(format: &OutputConfigFormat) -> Result<Self, Self::Error> {
        if format.format_type != "json_schema" {
            bail!("Unsupported output format type: {}", format.format_type);
        }

        let schema_str = serde_json::to_string(&format.schema)?;

        let json_schema = JsonSchemaDefinition::builder().schema(schema_str).build()?;

        let bedrock_format = BedrockOutputFormat::builder()
            .r#type(OutputFormatType::JsonSchema)
            .structure(OutputFormatStructure::JsonSchema(json_schema))
            .build()?;

        Ok(BedrockOutputConfig::builder()
            .text_format(bedrock_format)
            .build())
    }
}

impl TryFrom<&OutputConfig> for Option<BedrockOutputConfig> {
    type Error = anyhow::Error;

    fn try_from(config: &OutputConfig) -> Result<Self, Self::Error> {
        match config {
            OutputConfig::Format { format } => Ok(Some(BedrockOutputConfig::try_from(format)?)),
            _ => Ok(None),
        }
    }
}

pub fn additional_model_request_fields(
    thinking: Option<&Thinking>,
    output_config: Option<&OutputConfig>,
) -> Option<Document> {
    let effort_doc = match output_config {
        Some(OutputConfig::Effort { effort }) => Some(effort_document(effort)),
        _ => None,
    };

    [thinking.map(Document::from), effort_doc]
        .into_iter()
        .flatten()
        .reduce(|mut a, b| {
            if let (Some(map_a), Document::Object(map_b)) = (a.as_object_mut(), b) {
                map_a.extend(map_b);
            }
            a
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_output_config_deserializes_as_other() {
        let json = serde_json::json!({"foo": "bar"});
        let config: OutputConfig = serde_json::from_value(json).unwrap();
        assert!(matches!(config, OutputConfig::Other(_)));
    }

    #[test]
    fn unsupported_format_type_returns_error() {
        let format = OutputConfigFormat {
            format_type: "xml".into(),
            schema: serde_json::json!({}),
        };
        assert!(BedrockOutputConfig::try_from(&format).is_err());
    }

    #[test]
    fn valid_json_schema_produces_output_config() {
        let format = OutputConfigFormat {
            format_type: "json_schema".into(),
            schema: serde_json::json!({"type": "object"}),
        };
        assert!(BedrockOutputConfig::try_from(&format).is_ok());
    }

    #[test]
    fn additional_model_request_fields_merges_thinking_and_effort() {
        let thinking = Thinking::Enabled {
            budget_tokens: 1024,
        };
        let effort = OutputConfig::Effort {
            effort: "high".to_string(),
        };

        let result = additional_model_request_fields(Some(&thinking), Some(&effort));
        let Document::Object(map) = result.unwrap() else {
            panic!("expected Document::Object");
        };

        assert!(map.contains_key("thinking"));
        assert!(map.contains_key("output_config"));
        assert!(map.contains_key("anthropic_beta"));
    }

    #[test]
    fn additional_model_request_fields_merges_adaptive_thinking_and_effort() {
        let thinking = Thinking::Adaptive;
        let effort = OutputConfig::Effort {
            effort: "low".to_string(),
        };

        let result = additional_model_request_fields(Some(&thinking), Some(&effort));
        let Document::Object(map) = result.unwrap() else {
            panic!("expected Document::Object");
        };

        let Document::Object(thinking_map) = &map["thinking"] else {
            panic!("expected thinking to be Document::Object");
        };
        assert_eq!(
            thinking_map["type"],
            Document::String("adaptive".to_string())
        );

        assert!(map.contains_key("output_config"));
        assert!(map.contains_key("anthropic_beta"));
    }
}
