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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_to_bedrock_tools_without_cache() {
        let tool = Tool {
            cache_control: None,
            description: "Gets the weather".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            name: "get_weather".to_string(),
        };
        let bedrock_tools = Vec::<BedrockTool>::try_from(&tool).unwrap();
        assert_eq!(bedrock_tools.len(), 1);
        match &bedrock_tools[0] {
            BedrockTool::ToolSpec(spec) => {
                assert_eq!(spec.name(), "get_weather");
                assert_eq!(spec.description(), Some("Gets the weather"));
            }
            other => panic!("expected ToolSpec, got {:?}", other),
        }
    }

    #[test]
    fn tool_to_bedrock_tools_with_cache() {
        let tool = Tool {
            cache_control: Some(CacheControl {
                cache_control_type: "ephemeral".to_string(),
            }),
            description: "Gets the weather".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            name: "get_weather".to_string(),
        };
        let bedrock_tools = Vec::<BedrockTool>::try_from(&tool).unwrap();
        assert_eq!(bedrock_tools.len(), 2);
        match &bedrock_tools[0] {
            BedrockTool::ToolSpec(spec) => assert_eq!(spec.name(), "get_weather"),
            other => panic!("expected ToolSpec, got {:?}", other),
        }
        assert!(matches!(bedrock_tools[1], BedrockTool::CachePoint(_)));
    }

    #[test]
    fn tool_with_empty_description_is_none() {
        let tool = Tool {
            cache_control: None,
            description: "".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            name: "get_weather".to_string(),
        };
        let bedrock_tools = Vec::<BedrockTool>::try_from(&tool).unwrap();
        match &bedrock_tools[0] {
            BedrockTool::ToolSpec(spec) => assert!(spec.description().is_none()),
            other => panic!("expected ToolSpec, got {:?}", other),
        }
    }

    #[test]
    fn tools_to_bedrock_tools_empty_returns_none() {
        let tools: Vec<Tool> = vec![];
        let result = tools_to_bedrock_tools(&tools).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn tools_to_tool_configuration_with_cache() {
        let tools = vec![Tool {
            cache_control: Some(CacheControl {
                cache_control_type: "ephemeral".to_string(),
            }),
            description: "A tool".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            name: "my_tool".to_string(),
        }];
        let config = tools_to_tool_configuration(&tools).unwrap().unwrap();
        assert_eq!(config.tools().len(), 2);
        match &config.tools()[0] {
            BedrockTool::ToolSpec(spec) => assert_eq!(spec.name(), "my_tool"),
            other => panic!("expected ToolSpec, got {:?}", other),
        }
        assert!(matches!(config.tools()[1], BedrockTool::CachePoint(_)));
    }
}
