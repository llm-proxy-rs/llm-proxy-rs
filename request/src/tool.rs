use anyhow::Result;
use aws_sdk_bedrockruntime::types::{
    AnyToolChoice, AutoToolChoice, ImageBlock, ImageFormat, ImageSource, SpecificToolChoice,
    Tool as BedrockTool, ToolChoice as BedrockToolChoice, ToolConfiguration, ToolInputSchema,
    ToolResultBlock, ToolResultContentBlock, ToolSpecification, ToolUseBlock,
};
use base64::{Engine as _, engine::general_purpose};
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
                .map(|c| match c {
                    Content::Text { text } => ToolResultContentBlock::Text(text.clone()),
                    Content::ImageUrl { image_url } => {
                        // Parse data URL format: data:image/jpeg;base64,<base64_data>
                        let (format, base64_data) = if image_url.url.starts_with("data:image/") {
                            let parts: Vec<&str> = image_url.url.split(',').collect();
                            if parts.len() == 2 {
                                let header = parts[0];
                                let data = parts[1];

                                // Extract format from header like "data:image/jpeg;base64"
                                let format = if header.contains("jpeg") || header.contains("jpg") {
                                    ImageFormat::Jpeg
                                } else if header.contains("png") {
                                    ImageFormat::Png
                                } else if header.contains("gif") {
                                    ImageFormat::Gif
                                } else if header.contains("webp") {
                                    ImageFormat::Webp
                                } else {
                                    ImageFormat::Jpeg // default
                                };

                                (format, data.to_string())
                            } else {
                                (ImageFormat::Jpeg, image_url.url.clone())
                            }
                        } else {
                            // Assume it's raw base64 data
                            (ImageFormat::Jpeg, image_url.url.clone())
                        };

                        // Decode base64 data
                        let image_bytes = match general_purpose::STANDARD.decode(&base64_data) {
                            Ok(bytes) => bytes,
                            Err(_) => {
                                return ToolResultContentBlock::Text(
                                    "Error: Invalid base64 image data".to_string(),
                                );
                            }
                        };

                        let image_block = match ImageBlock::builder()
                            .format(format)
                            .source(ImageSource::Bytes(image_bytes.into()))
                            .build()
                        {
                            Ok(block) => block,
                            Err(_) => {
                                return ToolResultContentBlock::Text(
                                    "Error: Failed to create image block".to_string(),
                                );
                            }
                        };

                        ToolResultContentBlock::Image(image_block)
                    }
                })
                .collect(),
        }
    }
}

impl TryFrom<&Message> for ToolResultBlock {
    type Error = anyhow::Error;

    fn try_from(message: &Message) -> Result<Self, Self::Error> {
        Ok(ToolResultBlock::builder()
            .set_tool_use_id(message.tool_call_id.clone())
            .set_content(message.contents.as_ref().map(|contents| contents.into()))
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
        let tool_spec = ToolSpecification::builder()
            .name(&tool.function.name)
            .set_description(tool.function.description.clone())
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

pub fn value_to_document(value: &serde_json::Value) -> aws_smithy_types::Document {
    match value {
        serde_json::Value::Null => aws_smithy_types::Document::Null,
        serde_json::Value::Bool(b) => aws_smithy_types::Document::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                aws_smithy_types::Document::Number(if i >= 0 {
                    aws_smithy_types::Number::PosInt(i as u64)
                } else {
                    aws_smithy_types::Number::NegInt(i)
                })
            } else {
                aws_smithy_types::Document::Number(aws_smithy_types::Number::Float(
                    n.as_f64().unwrap_or(0.0),
                ))
            }
        }
        serde_json::Value::String(s) => aws_smithy_types::Document::String(s.clone()),
        serde_json::Value::Array(a) => {
            aws_smithy_types::Document::Array(a.iter().map(value_to_document).collect())
        }
        serde_json::Value::Object(o) => aws_smithy_types::Document::Object(
            o.iter()
                .map(|(k, v)| (k.clone(), value_to_document(v)))
                .collect(),
        ),
    }
}
