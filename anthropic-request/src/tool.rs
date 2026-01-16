use anyhow::Result;
use aws_sdk_bedrockruntime::types::{
    AnyToolChoice, AutoToolChoice, SpecificToolChoice, Tool as BedrockTool,
    ToolChoice as BedrockToolChoice, ToolInputSchema, ToolSpecification,
};
use serde::{Deserialize, Serialize};

use crate::value_to_document;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Tool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum ToolChoice {
    String(String), // "auto", "any", "tool"
    Object {
        #[serde(rename = "type")]
        choice_type: String, // "auto", "any", "tool"
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>, // Required if type is "tool"
    },
}

// Trait implementations for conversions to Bedrock types

impl TryFrom<&Tool> for BedrockTool {
    type Error = anyhow::Error;

    fn try_from(tool: &Tool) -> Result<Self, Self::Error> {
        let tool_spec = ToolSpecification::builder()
            .name(&tool.name)
            .set_description(tool.description.clone())
            .input_schema(ToolInputSchema::Json(value_to_document(&tool.input_schema)))
            .build()?;

        Ok(BedrockTool::ToolSpec(tool_spec))
    }
}

impl TryFrom<&ToolChoice> for Option<BedrockToolChoice> {
    type Error = anyhow::Error;

    fn try_from(tool_choice: &ToolChoice) -> Result<Self, Self::Error> {
        match tool_choice {
            ToolChoice::String(s) => match s.as_str() {
                "auto" => Ok(Some(BedrockToolChoice::Auto(
                    AutoToolChoice::builder().build(),
                ))),
                "any" => Ok(Some(BedrockToolChoice::Any(
                    AnyToolChoice::builder().build(),
                ))),
                _ => Ok(Some(BedrockToolChoice::Auto(
                    AutoToolChoice::builder().build(),
                ))),
            },
            ToolChoice::Object { choice_type, name } => match choice_type.as_str() {
                "tool" => {
                    if let Some(name) = name {
                        Ok(Some(BedrockToolChoice::Tool(
                            SpecificToolChoice::builder().name(name).build()?,
                        )))
                    } else {
                        Ok(Some(BedrockToolChoice::Auto(
                            AutoToolChoice::builder().build(),
                        )))
                    }
                }
                "any" => Ok(Some(BedrockToolChoice::Any(
                    AnyToolChoice::builder().build(),
                ))),
                _ => Ok(Some(BedrockToolChoice::Auto(
                    AutoToolChoice::builder().build(),
                ))),
            },
        }
    }
}
