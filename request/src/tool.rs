use anyhow::Result;
use aws_sdk_bedrockruntime::types::{
    AnyToolChoice, AutoToolChoice, ImageBlock, SpecificToolChoice, Tool as BedrockTool,
    ToolChoice as BedrockToolChoice, ToolConfiguration, ToolInputSchema, ToolResultBlock,
    ToolResultContentBlock, ToolSpecification, ToolUseBlock,
};
use common::value_to_document;
use serde::{Deserialize, Serialize};

use crate::{ChatCompletionsRequest, Content, Contents, Message};

#[derive(Debug, Deserialize, Serialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunction,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolFunction {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ToolChoice {
    String(String),
    Object {
        #[serde(rename = "type")]
        tool_type: String,
        function: ToolChoiceFunction,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolChoiceFunction {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

impl From<&Contents> for Vec<ToolResultContentBlock> {
    fn from(contents: &Contents) -> Self {
        match contents {
            Contents::String(s) => {
                vec![ToolResultContentBlock::Text(s.clone())]
            }
            Contents::Array(a) => a
                .iter()
                .filter_map(|c| match c {
                    Content::Text { text } => Some(ToolResultContentBlock::Text(text.clone())),
                    Content::ImageUrl { image_url } => {
                        Option::<ImageBlock>::from(image_url).map(ToolResultContentBlock::Image)
                    }
                })
                .collect(),
        }
    }
}

impl TryFrom<&Message> for ToolResultBlock {
    type Error = anyhow::Error;

    fn try_from(message: &Message) -> Result<Self, Self::Error> {
        let Message::Tool {
            contents,
            tool_call_id,
        } = message
        else {
            unreachable!()
        };

        Ok(ToolResultBlock::builder()
            .set_tool_use_id(tool_call_id.clone())
            .set_content(contents.as_ref().map(|contents| contents.into()))
            .build()?)
    }
}

impl TryFrom<&ToolCall> for ToolUseBlock {
    type Error = anyhow::Error;

    fn try_from(tool_call: &ToolCall) -> Result<Self, Self::Error> {
        let input = serde_json::from_str(&tool_call.function.arguments)
            .map(|value| value_to_document(&value))?;

        Ok(ToolUseBlock::builder()
            .tool_use_id(&tool_call.id)
            .name(&tool_call.function.name)
            .input(input)
            .build()?)
    }
}

impl TryFrom<&Tool> for BedrockTool {
    type Error = anyhow::Error;

    fn try_from(tool: &Tool) -> Result<Self, Self::Error> {
        let description = tool
            .function
            .description
            .as_ref()
            .filter(|d| !d.trim().is_empty())
            .cloned();

        let tool_spec = ToolSpecification::builder()
            .name(&tool.function.name)
            .set_description(description)
            .input_schema(ToolInputSchema::Json(value_to_document(
                &tool.function.parameters,
            )))
            .build()?;

        Ok(BedrockTool::ToolSpec(tool_spec))
    }
}

impl TryFrom<&ToolChoice> for Option<BedrockToolChoice> {
    type Error = anyhow::Error;

    fn try_from(tool_choice: &ToolChoice) -> Result<Self, Self::Error> {
        match tool_choice {
            ToolChoice::String(s) => match s.as_str() {
                "none" => Ok(None),
                "required" => Ok(Some(BedrockToolChoice::Any(
                    AnyToolChoice::builder().build(),
                ))),
                _ => Ok(Some(BedrockToolChoice::Auto(
                    AutoToolChoice::builder().build(),
                ))),
            },
            ToolChoice::Object { function, .. } => Ok(Some(BedrockToolChoice::Tool(
                SpecificToolChoice::builder().name(&function.name).build()?,
            ))),
        }
    }
}

impl TryFrom<&ChatCompletionsRequest> for Option<ToolConfiguration> {
    type Error = anyhow::Error;

    fn try_from(request: &ChatCompletionsRequest) -> Result<Self, Self::Error> {
        if request.tools.is_none() && request.tool_choice.is_none() {
            return Ok(None);
        }

        let mut builder = ToolConfiguration::builder();

        if let Some(tools) = &request.tools {
            for tool in tools {
                let bedrock_tool = BedrockTool::try_from(tool)?;
                builder = builder.tools(bedrock_tool);
            }
        }

        if let Some(tool_choice) = &request.tool_choice {
            let bedrock_tool_choice = Option::<BedrockToolChoice>::try_from(tool_choice)?;
            builder = builder.set_tool_choice(bedrock_tool_choice);
        }

        Ok(Some(builder.build()?))
    }
}
