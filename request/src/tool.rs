use anyhow::Result;
use aws_sdk_bedrockruntime::types::{
    AnyToolChoice, AutoToolChoice, ImageBlock, SpecificToolChoice, Tool as BedrockTool,
    ToolChoice as BedrockToolChoice, ToolConfiguration, ToolInputSchema, ToolResultBlock,
    ToolResultContentBlock, ToolSpecification, ToolUseBlock,
};
use serde::{Deserialize, Serialize};

use crate::{ChatCompletionsRequest, Content, Contents, Message};

#[derive(Debug, Deserialize, Serialize)]
pub struct Tool {
    pub function: ToolFunction,
    #[serde(rename = "type")]
    pub tool_type: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ToolFunction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub name: String,
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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionCall,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
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
            .tool_use_id(tool_call_id)
            .set_content(Some(contents.into()))
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

impl Tool {
    pub fn builder() -> ToolBuilder {
        ToolBuilder::default()
    }
}

impl ToolFunction {
    pub fn builder() -> ToolFunctionBuilder {
        ToolFunctionBuilder::default()
    }
}

#[derive(Default)]
pub struct ToolBuilder {
    function: ToolFunction,
    tool_type: String,
}

#[derive(Default)]
pub struct ToolFunctionBuilder {
    description: Option<String>,
    name: String,
    parameters: serde_json::Value,
}

impl ToolBuilder {
    pub fn function(mut self, function: ToolFunction) -> Self {
        self.function = function;
        self
    }

    pub fn tool_type(mut self, tool_type: String) -> Self {
        self.tool_type = tool_type;
        self
    }

    pub fn build(self) -> Tool {
        Tool {
            function: self.function,
            tool_type: self.tool_type,
        }
    }
}

impl ToolFunctionBuilder {
    pub fn description(mut self, description: Option<String>) -> Self {
        self.description = description;
        self
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    pub fn parameters(mut self, parameters: serde_json::Value) -> Self {
        self.parameters = parameters;
        self
    }

    pub fn build(self) -> ToolFunction {
        ToolFunction {
            description: self.description,
            name: self.name,
            parameters: self.parameters,
        }
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
