use aws_sdk_bedrockruntime::types::{
    AutoToolChoice, CachePointBlock, Tool as BedrockTool, ToolChoice, ToolConfiguration,
    ToolInputSchema, ToolSpecification,
};
use common::value_to_document;
use serde::{Deserialize, Serialize};

use crate::cache_control::CacheControl;

#[derive(Debug, Deserialize, Serialize)]
pub struct Tool {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub name: String,
}

impl TryFrom<&Tool> for Vec<BedrockTool> {
    type Error = anyhow::Error;

    fn try_from(tool: &Tool) -> Result<Self, Self::Error> {
        let mut tools = vec![BedrockTool::ToolSpec(
            ToolSpecification::builder()
                .name(&tool.name)
                .set_description((!tool.description.is_empty()).then(|| tool.description.clone()))
                .input_schema(ToolInputSchema::Json(value_to_document(&tool.input_schema)))
                .build()?,
        )];

        if let Some(cache_control) = &tool.cache_control {
            let cache_point = CachePointBlock::try_from(cache_control)?;
            tools.push(BedrockTool::CachePoint(cache_point));
        }

        Ok(tools)
    }
}

pub fn tools_to_bedrock_tools(tools: &[Tool]) -> anyhow::Result<Option<Vec<BedrockTool>>> {
    let bedrock_tools: Vec<BedrockTool> = tools
        .iter()
        .map(Vec::<BedrockTool>::try_from)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();

    Ok(if bedrock_tools.is_empty() {
        None
    } else {
        Some(bedrock_tools)
    })
}

pub fn tools_to_tool_configuration(tools: &[Tool]) -> anyhow::Result<Option<ToolConfiguration>> {
    let bedrock_tools = tools_to_bedrock_tools(tools)?;

    bedrock_tools
        .map(|tools| {
            ToolConfiguration::builder()
                .set_tools(Some(tools))
                .tool_choice(ToolChoice::Auto(AutoToolChoice::builder().build()))
                .build()
                .map_err(Into::into)
        })
        .transpose()
}
