use aws_sdk_bedrockruntime::types::{
    AutoToolChoice, CachePointBlock, Tool as BedrockTool, ToolChoice, ToolConfiguration,
    ToolInputSchema, ToolSpecification,
};
use common::value_to_document;
use serde::{Deserialize, Serialize};

use crate::cache_control::CacheControl;

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Tool {
    Custom {
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        input_schema: serde_json::Value,
        name: String,
    },
    Server(serde_json::Value),
}

impl TryFrom<&Tool> for Option<Vec<BedrockTool>> {
    type Error = anyhow::Error;

    fn try_from(tool: &Tool) -> Result<Self, Self::Error> {
        match tool {
            Tool::Custom {
                cache_control,
                description,
                input_schema,
                name,
            } => {
                let mut tools = vec![BedrockTool::ToolSpec(
                    ToolSpecification::builder()
                        .name(name)
                        .set_description(
                            description
                                .as_deref()
                                .filter(|d| !d.is_empty())
                                .map(str::to_owned),
                        )
                        .input_schema(ToolInputSchema::Json(value_to_document(input_schema)))
                        .build()?,
                )];

                if let Some(cc) = cache_control {
                    tools.push(BedrockTool::CachePoint(CachePointBlock::try_from(cc)?));
                }

                Ok(Some(tools))
            }
            Tool::Server(_) => Ok(None),
        }
    }
}

pub fn build_bedrock_tools(tools: &[Tool]) -> anyhow::Result<Option<Vec<BedrockTool>>> {
    let bedrock_tools: Vec<BedrockTool> = tools
        .iter()
        .map(Option::<Vec<_>>::try_from)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .flatten()
        .collect();

    Ok(if bedrock_tools.is_empty() {
        None
    } else {
        Some(bedrock_tools)
    })
}

pub fn build_tool_configuration(tools: &[Tool]) -> anyhow::Result<Option<ToolConfiguration>> {
    let bedrock_tools = build_bedrock_tools(tools)?;

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
        let tool = Tool::Custom {
            cache_control: None,
            description: Some("Gets the weather".to_string()),
            input_schema: serde_json::json!({"type": "object"}),
            name: "get_weather".to_string(),
        };
        let bedrock_tools = Option::<Vec<BedrockTool>>::try_from(&tool)
            .unwrap()
            .unwrap();
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
        let tool = Tool::Custom {
            cache_control: Some(CacheControl {
                cache_control_type: "ephemeral".to_string(),
                ttl: None,
            }),
            description: Some("Gets the weather".to_string()),
            input_schema: serde_json::json!({"type": "object"}),
            name: "get_weather".to_string(),
        };
        let bedrock_tools = Option::<Vec<BedrockTool>>::try_from(&tool)
            .unwrap()
            .unwrap();
        assert_eq!(bedrock_tools.len(), 2);
        match &bedrock_tools[0] {
            BedrockTool::ToolSpec(spec) => assert_eq!(spec.name(), "get_weather"),
            other => panic!("expected ToolSpec, got {:?}", other),
        }
        assert!(matches!(bedrock_tools[1], BedrockTool::CachePoint(_)));
    }

    #[test]
    fn tool_with_empty_description_is_none() {
        let tool = Tool::Custom {
            cache_control: None,
            description: Some("".to_string()),
            input_schema: serde_json::json!({"type": "object"}),
            name: "get_weather".to_string(),
        };
        let bedrock_tools = Option::<Vec<BedrockTool>>::try_from(&tool)
            .unwrap()
            .unwrap();
        match &bedrock_tools[0] {
            BedrockTool::ToolSpec(spec) => assert!(spec.description().is_none()),
            other => panic!("expected ToolSpec, got {:?}", other),
        }
    }

    #[test]
    fn build_bedrock_tools_empty_returns_none() {
        let tools: Vec<Tool> = vec![];
        let result = build_bedrock_tools(&tools).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn build_tool_configuration_with_cache() {
        let tools = vec![Tool::Custom {
            cache_control: Some(CacheControl {
                cache_control_type: "ephemeral".to_string(),
                ttl: None,
            }),
            description: Some("A tool".to_string()),
            input_schema: serde_json::json!({"type": "object"}),
            name: "my_tool".to_string(),
        }];
        let config = build_tool_configuration(&tools).unwrap().unwrap();
        assert_eq!(config.tools().len(), 2);
        match &config.tools()[0] {
            BedrockTool::ToolSpec(spec) => assert_eq!(spec.name(), "my_tool"),
            other => panic!("expected ToolSpec, got {:?}", other),
        }
        assert!(matches!(config.tools()[1], BedrockTool::CachePoint(_)));
    }
}
