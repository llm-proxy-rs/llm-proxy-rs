use anyhow::Result;
use aws_sdk_bedrockruntime::types::{
    AnyToolChoice, AutoToolChoice, Message, SpecificToolChoice, SystemContentBlock, Tool,
    ToolChoice, ToolConfiguration, ToolInputSchema, ToolSpecification,
};
use aws_smithy_types::Document;
use request::{ChatCompletionsRequest, OpenAITool, OpenAIToolChoice, Role};
use serde_json::Value;

pub struct BedrockChatCompletion {
    pub model_id: String,
    pub system_content_blocks: Vec<SystemContentBlock>,
    pub messages: Vec<Message>,
    pub tool_config: Option<ToolConfiguration>,
}

pub fn process_chat_completions_request_to_bedrock_chat_completion(
    request: &ChatCompletionsRequest,
) -> Result<BedrockChatCompletion> {
    let mut system_content_blocks = Vec::new();
    let mut messages = Vec::new();
    let model_id = request.model.clone();

    for request_message in &request.messages {
        match request_message.role {
            Role::Assistant | Role::User => {
                if let Ok(message) = Message::try_from(request_message) {
                    messages.push(message);
                }
            }
            Role::System => {
                let new_system_content_blocks: Vec<SystemContentBlock> =
                    (&request_message.contents).into();
                system_content_blocks.extend(new_system_content_blocks);
            }
            Role::Tool => {}
        }
    }

    let tool_config = request
        .tools
        .as_ref()
        .map(|tools| convert_openai_tools_to_bedrock_tool_config(tools, &request.tool_choice))
        .transpose()?;

    Ok(BedrockChatCompletion {
        model_id,
        system_content_blocks,
        messages,
        tool_config,
    })
}

fn convert_openai_tools_to_bedrock_tool_config(
    openai_tools: &[OpenAITool],
    openai_tool_choice: &Option<OpenAIToolChoice>,
) -> Result<ToolConfiguration> {
    let mut builder = ToolConfiguration::builder();

    for openai_tool in openai_tools {
        let tool_spec = ToolSpecification::builder()
            .name(&openai_tool.function.name)
            .set_description(openai_tool.function.description.clone())
            .input_schema(ToolInputSchema::Json(convert_value_to_document(
                &openai_tool.function.parameters,
            )))
            .build()?;

        builder = builder.tools(Tool::ToolSpec(tool_spec));
    }

    if let Some(openai_tool_choice) = openai_tool_choice {
        let bedrock_tool_choice = match openai_tool_choice {
            OpenAIToolChoice::String(s) => match s.as_str() {
                "none" => None,
                "required" => Some(ToolChoice::Any(AnyToolChoice::builder().build())),
                _ => Some(ToolChoice::Auto(AutoToolChoice::builder().build())),
            },
            OpenAIToolChoice::Object { function, .. } => Some(ToolChoice::Tool(
                SpecificToolChoice::builder().name(&function.name).build()?,
            )),
        };
        builder = builder.set_tool_choice(bedrock_tool_choice);
    }

    Ok(builder.build()?)
}

fn convert_value_to_document(value: &Value) -> Document {
    match value {
        Value::Null => Document::Null,
        Value::Bool(b) => Document::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Document::Number(if i >= 0 {
                    aws_smithy_types::Number::PosInt(i as u64)
                } else {
                    aws_smithy_types::Number::NegInt(i)
                })
            } else {
                Document::Number(aws_smithy_types::Number::Float(n.as_f64().unwrap_or(0.0)))
            }
        }
        Value::String(s) => Document::String(s.clone()),
        Value::Array(a) => Document::Array(a.iter().map(convert_value_to_document).collect()),
        Value::Object(o) => Document::Object(
            o.iter()
                .map(|(k, v)| (k.clone(), convert_value_to_document(v)))
                .collect(),
        ),
    }
}
