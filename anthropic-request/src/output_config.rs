use anyhow::{Result, bail};
use aws_sdk_bedrockruntime::types::{
    JsonSchemaDefinition, OutputConfig as BedrockOutputConfig, OutputFormat as BedrockOutputFormat,
    OutputFormatStructure, OutputFormatType,
};
use aws_smithy_types::Document;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum OutputConfig {
    Format { format: OutputConfigFormat },
    Effort { effort: String },
    Other(serde_json::Value),
}

pub fn get_output_config_effort_document(effort: &str) -> Document {
    Document::Object(
        [(
            "output_config".to_string(),
            Document::Object(
                [("effort".to_string(), Document::String(effort.to_string()))]
                    .into_iter()
                    .collect(),
            ),
        )]
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
}
